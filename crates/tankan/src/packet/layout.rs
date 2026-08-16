//! packet のレイアウト。
//!
//! フィールドを bitsPerRow 境界で行ブロックへ分割し（行跨ぎはラベルを両ブロックに
//! 複製 = mermaid と同じ）、格子座標へ焼き込む。本家はビット番号を rect 上端に
//! 重ね書きするが、1 行目が viewBox の外へ出るため tankan では各行の上に
//! ビット番号帯（`bit_fs * 1.2`）を明示確保する（本家に寄せた合理的仕様）。

use crate::Options;
use crate::packet::model::PacketDiagram;

const MARGIN: f32 = 16.0;

pub(crate) struct Layout {
    pub width: f32,
    pub height: f32,
    pub line_h: f32,
    pub title: Option<String>,
    /// 行分割済みブロック（フィールド順・行順）
    pub blocks: Vec<Block>,
    pub show_bits: bool,
    /// ビット番号のフォントサイズ（fs * 0.75）
    pub bit_fs: f32,
}

pub(crate) struct Block {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub label: String,
    /// このブロックが表す絶対ビット範囲（ビット番号の描画用）
    pub bit_start: u32,
    pub bit_end: u32,
}

pub(crate) fn layout(diagram: &PacketDiagram, options: &Options) -> Layout {
    let fs = options.font_size;
    let cfg = &diagram.config;
    let bpr = cfg.bits_per_row;

    let bit_fs = fs * 0.75;
    let bit_band = if cfg.show_bits { bit_fs * 1.2 } else { 0.0 };
    let row_pitch = bit_band + cfg.row_height + cfg.padding_y;

    let title = diagram.title.clone();
    let title_h = if title.is_some() { fs * 2.0 } else { 0.0 };
    let content_top = MARGIN + title_h;

    // フィールド → 行ブロック分割（行内は列位置に比例した x）
    let mut blocks = Vec::new();
    for field in &diagram.fields {
        let mut cur = field.start;
        while cur <= field.end {
            let row = cur / bpr;
            let seg_end = field.end.min((row + 1) * bpr - 1);
            let col = cur % bpr;
            let ncols = seg_end - cur + 1;
            blocks.push(Block {
                x: MARGIN + col as f32 * cfg.bit_width + cfg.padding_x,
                y: content_top + row as f32 * row_pitch + bit_band,
                w: ncols as f32 * cfg.bit_width - cfg.padding_x,
                h: cfg.row_height,
                label: field.label.clone(),
                bit_start: cur,
                bit_end: seg_end,
            });
            cur = seg_end + 1;
        }
    }

    // パース時に fields 非空が保証されている
    let total_bits = diagram.fields.last().map_or(0, |f| f.end + 1);
    let nrows = total_bits.div_ceil(bpr);
    let used_cols = total_bits.min(bpr);

    Layout {
        width: 2.0 * MARGIN + used_cols as f32 * cfg.bit_width,
        height: content_top + nrows as f32 * row_pitch - cfg.padding_y + MARGIN,
        line_h: fs * 1.4,
        title,
        blocks,
        show_bits: cfg.show_bits,
        bit_fs,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packet::parser::parse;

    fn lay(src: &str) -> Layout {
        layout(&parse(src).unwrap(), &Options::default())
    }

    #[test]
    fn 行内のフィールドは_1_ブロックになる() {
        let l = lay("packet\n0-15: \"a\"\n16-31: \"b\"\n");
        assert_eq!(l.blocks.len(), 2);
        assert_eq!(l.blocks[0].y, l.blocks[1].y, "同じ行");
        assert_eq!((l.blocks[0].bit_start, l.blocks[0].bit_end), (0, 15));
    }

    #[test]
    fn 行境界を跨ぐフィールドは行ごとに分割される() {
        let l = lay("packet\n0-23: \"head\"\n24-39: \"crossing\"\n");
        // 24-39 は 24-31（1 行目）と 32-39（2 行目）へ分割
        assert_eq!(l.blocks.len(), 3);
        let (b1, b2) = (&l.blocks[1], &l.blocks[2]);
        assert_eq!((b1.bit_start, b1.bit_end), (24, 31));
        assert_eq!((b2.bit_start, b2.bit_end), (32, 39));
        assert_eq!(b1.label, b2.label, "ラベルは両ブロックに複製");
        assert!(b2.y > b1.y, "分割後は次の行");
        assert_eq!(b2.x, l.blocks[0].x, "次行は左端（col 0）から");
    }

    #[test]
    fn ブロックの_x_はビット位置に比例する() {
        let l = lay("packet\n0-7: \"a\"\n8-15: \"b\"\n16-31: \"c\"\n");
        let bw = 32.0;
        assert_eq!(l.blocks[1].x - l.blocks[0].x, 8.0 * bw);
        assert_eq!(l.blocks[2].x - l.blocks[0].x, 16.0 * bw);
        assert_eq!(
            l.blocks[2].w - l.blocks[0].w,
            8.0 * bw,
            "16 ビット幅は 8 ビット幅の +8bw"
        );
    }

    #[test]
    fn タイトル有無で高さが変わる() {
        let without = lay("packet\n0-15: \"a\"\n");
        let with = lay("---\ntitle: \"T\"\n---\npacket\n0-15: \"a\"\n");
        assert!(with.height > without.height);
        assert_eq!(with.title.as_deref(), Some("T"));
    }

    #[test]
    fn 総ビット数が_1_行未満なら幅が縮む() {
        let small = lay("packet\n0-7: \"a\"\n");
        let full = lay("packet\n0-31: \"a\"\n");
        assert!(small.width < full.width);
        assert_eq!(full.width, 2.0 * 16.0 + 32.0 * 32.0);
    }

    #[test]
    fn bits_per_row_16_で行数が倍になる() {
        let l16 = lay("---\nconfig:\n  packet:\n    bitsPerRow: 16\n---\npacket\n0-31: \"a\"\n");
        let l32 = lay("packet\n0-31: \"a\"\n");
        assert_eq!(l16.blocks.len(), 2, "16bit/行 では 2 ブロックへ分割");
        assert_eq!(l32.blocks.len(), 1);
        assert!(l16.height > l32.height);
        assert!(l16.width < l32.width);
    }

    #[test]
    fn show_bits_false_で高さが縮む() {
        let on = lay("packet\n0-31: \"a\"\n");
        let off = lay("---\nconfig:\n  packet:\n    showBits: false\n---\npacket\n0-31: \"a\"\n");
        assert!(!off.show_bits);
        assert!(off.height < on.height);
    }
}
