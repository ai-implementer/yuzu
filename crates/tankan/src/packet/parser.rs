//! packet の行指向パーサ。
//!
//! 対応構文（mermaid 互換）:
//! - ヘッダ: `packet` / 旧 `packet-beta`
//! - フィールド行 3 形式: 範囲 `0-15: "Source Port"` / 単一ビット `106: "URG"` /
//!   相対 `+16: "Label"`（直前フィールドの終端 +1 から count ビット分。先頭なら 0 から）
//! - フィールドはビット 0 から連続（隙間・重複はエラー = mermaid と同じ検証）
//! - `%%` コメント・`%%{init}%%` ディレクティブ・YAML frontmatter（title を拾う）
//! - frontmatter の `config.packet`（bitsPerRow / bitWidth / rowHeight / showBits /
//!   paddingX / paddingY）をインデントベースで最小パースする。未知キーは無視、
//!   インライン形式（`config: {...}`）は非対応 = 読み飛ばして既定値
//! - `accTitle:` / `accDescr:` は受理して無視する
//!
//! tankan の寛容拡張（本家はエラー）: ラベルの引用符省略・`'` 引用、本文の
//! `title <テキスト>` 行も受理する。frontmatter / ラベルの引用符は 1 層だけ剥がす。
//! tankan 独自の上限として総ビット数は 4096 まで（巨大入力でのメモリ暴走防止）。

use crate::error::Error;
use crate::kind::trim_line;
use crate::packet::model::{Field, PacketDiagram};

/// 総ビット数の上限（tankan 独自の安全弁。TCP=256bit、IPv6 拡張でも余裕）
const MAX_BITS: u32 = 4096;

pub(crate) fn parse(source: &str) -> Result<PacketDiagram, Error> {
    let mut diagram = PacketDiagram::default();

    let mut in_directive = false;
    let mut in_frontmatter = false;
    let mut seen_header = false;
    let mut first_content = true;
    // frontmatter の config.packet 用の状態（インデントはスペース数で追う）
    let mut in_config = false;
    let mut packet_indent: Option<usize> = None;
    // config キーを最後に適用した行（キー単体は正しくても組み合わせが不正な場合の報告用）
    let mut config_key_line = 1;

    for (idx, raw) in source.lines().enumerate() {
        let line_no = idx + 1;
        let line = trim_line(raw);
        if line.is_empty() {
            continue;
        }
        if in_directive {
            if line.ends_with("}%%") {
                in_directive = false;
            }
            continue;
        }
        if in_frontmatter {
            if line == "---" {
                in_frontmatter = false;
                continue;
            }
            let indent = raw.len() - raw.trim_start_matches(' ').len();
            // インデントが戻ったらブロックを抜ける
            if packet_indent.is_some_and(|pi| indent <= pi) {
                packet_indent = None;
            }
            if in_config && indent == 0 {
                in_config = false;
            }
            if indent == 0 {
                if let Some(t) = line.strip_prefix("title:") {
                    diagram.title = Some(unquote(t.trim()).to_string());
                } else if line == "config:" {
                    in_config = true;
                }
                continue;
            }
            if in_config && packet_indent.is_none() {
                if line == "packet:" {
                    packet_indent = Some(indent);
                }
                continue;
            }
            if packet_indent.is_some() {
                if let Some((key, value)) = line.split_once(':') {
                    apply_config(&mut diagram, key.trim(), value.trim(), line_no)?;
                    config_key_line = line_no;
                }
            }
            continue;
        }
        if first_content && line == "---" {
            in_frontmatter = true;
            first_content = false;
            continue;
        }
        first_content = false;
        if line.starts_with("%%{") {
            if !line.ends_with("}%%") {
                in_directive = true;
            }
            continue;
        }
        if line.starts_with("%%") {
            continue;
        }

        if !seen_header {
            if line != "packet" && line != "packet-beta" {
                return Err(Error::Parse {
                    line: line_no,
                    message: "packet ヘッダがありません".to_string(),
                });
            }
            seen_header = true;
            continue;
        }

        if let Some(t) = line.strip_prefix("title ") {
            diagram.title = Some(unquote(t.trim()).to_string());
            continue;
        }
        if line.starts_with("accTitle") || line.starts_with("accDescr") {
            continue; // アクセシビリティ行は受理して無視（他図種と同じ）
        }

        // フィールド行 `範囲: "ラベル"`
        let Some((spec, label_part)) = line.split_once(':') else {
            return Err(Error::Parse {
                line: line_no,
                message: "フィールド行に `:` がありません（例: `0-15: \"Source Port\"`）"
                    .to_string(),
            });
        };
        let prev_end = diagram.fields.last().map(|f| f.end);
        let (start, end) = parse_range(spec.trim(), prev_end, line_no)?;
        if end < start {
            return Err(Error::Parse {
                line: line_no,
                message: format!("ビット範囲の終端 {end} が開始 {start} より前です"),
            });
        }
        let expected = prev_end.map_or(0, |e| e + 1);
        if start > expected {
            return Err(Error::Parse {
                line: line_no,
                message: format!(
                    "ビット {expected}〜{} が抜けています（フィールドはビット 0 から連続させる）",
                    start - 1
                ),
            });
        }
        if start < expected {
            return Err(Error::Parse {
                line: line_no,
                message: format!(
                    "ビット {start} は直前のフィールドと重複しています（次は {expected} から）"
                ),
            });
        }
        if end >= MAX_BITS {
            return Err(Error::Parse {
                line: line_no,
                message: format!("総ビット数の上限（{MAX_BITS}）を超えています: 終端ビット {end}"),
            });
        }
        diagram.fields.push(Field {
            start,
            end,
            label: unquote(trim_line(label_part)).to_string(),
        });
    }

    // キー単体は正しくても組み合わせで壊れる設定を弾く
    // （最小ブロック = 1 ビット幅の rect が負幅にならないこと）
    if diagram.config.padding_x >= diagram.config.bit_width {
        return Err(Error::Parse {
            line: config_key_line,
            message: format!(
                "frontmatter の packet 設定が不正です: paddingX（{}）は bitWidth（{}）より小さくする必要があります",
                diagram.config.padding_x, diagram.config.bit_width
            ),
        });
    }
    if !seen_header {
        return Err(Error::Parse {
            line: 1,
            message: "packet ヘッダがありません".to_string(),
        });
    }
    if diagram.fields.is_empty() {
        return Err(Error::Parse {
            line: 1,
            message: "フィールド行がありません".to_string(),
        });
    }
    Ok(diagram)
}

/// 範囲部の解決（`+N` 相対 → `a-b` 範囲 → 単一数値の順に判定）
fn parse_range(spec: &str, prev_end: Option<u32>, line_no: usize) -> Result<(u32, u32), Error> {
    let bad = || Error::Parse {
        line: line_no,
        message: format!("ビット範囲を解釈できません: `{spec}`"),
    };
    if let Some(rest) = spec.strip_prefix('+') {
        let count: u32 = rest.trim().parse().map_err(|_| bad())?;
        if count == 0 {
            return Err(Error::Parse {
                line: line_no,
                message: format!("相対指定は 1 ビット以上が必要です: `{spec}`"),
            });
        }
        let start = prev_end.map_or(0, |e| e + 1);
        let end = start.checked_add(count - 1).ok_or_else(bad)?;
        return Ok((start, end));
    }
    if let Some((a, b)) = spec.split_once('-') {
        let start: u32 = a.trim().parse().map_err(|_| bad())?;
        let end: u32 = b.trim().parse().map_err(|_| bad())?;
        return Ok((start, end));
    }
    let n: u32 = spec.trim().parse().map_err(|_| bad())?;
    Ok((n, n))
}

/// `config.packet` の 1 キーを反映する。未知キーは無視、値の型不正のみエラー
fn apply_config(
    diagram: &mut PacketDiagram,
    key: &str,
    value: &str,
    line_no: usize,
) -> Result<(), Error> {
    let value = unquote(value);
    let bad = || Error::Parse {
        line: line_no,
        message: format!("frontmatter の packet 設定 `{key}` の値が不正です: `{value}`"),
    };
    // 長さ系の上限は 4096px（巨大値で座標が壊れるのを防ぐ tankan 独自の安全弁）
    let parse_len = |min: f32| -> Result<f32, Error> {
        let v: f32 = value.parse().map_err(|_| bad())?;
        if !v.is_finite() || v < min || v > 4096.0 {
            return Err(bad());
        }
        Ok(v)
    };
    let cfg = &mut diagram.config;
    match key {
        "bitsPerRow" => {
            let n: u32 = value.parse().map_err(|_| bad())?;
            if !(1..=256).contains(&n) {
                return Err(bad());
            }
            cfg.bits_per_row = n;
        }
        "bitWidth" => cfg.bit_width = parse_len(1.0)?,
        "rowHeight" => cfg.row_height = parse_len(1.0)?,
        "paddingX" => cfg.padding_x = parse_len(0.0)?,
        "paddingY" => cfg.padding_y = parse_len(0.0)?,
        "showBits" => {
            cfg.show_bits = match value {
                "true" => true,
                "false" => false,
                _ => return Err(bad()),
            };
        }
        _ => {} // 未知キーは無視（mermaid の他設定と将来キーを壊さない）
    }
    Ok(())
}

/// 両端が同じ引用符（`"` / `'`）なら 1 層だけ剥がす
fn unquote(s: &str) -> &str {
    let b = s.as_bytes();
    if b.len() >= 2 {
        let (first, last) = (b[0], b[b.len() - 1]);
        if (first == b'"' && last == b'"') || (first == b'\'' && last == b'\'') {
            return &s[1..s.len() - 1];
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::parse;

    #[test]
    fn 基本形をパースできる() {
        let d = parse("packet\n0-15: \"Source Port\"\n16-31: \"Destination Port\"\n").unwrap();
        assert_eq!(d.title, None);
        assert_eq!(d.fields.len(), 2);
        assert_eq!(
            (
                d.fields[0].start,
                d.fields[0].end,
                d.fields[0].label.as_str()
            ),
            (0, 15, "Source Port")
        );
        assert_eq!((d.fields[1].start, d.fields[1].end), (16, 31));
    }

    #[test]
    fn 単一ビットと範囲が混在できる() {
        let d = parse("packet\n0-105: \"Head\"\n106: \"URG\"\n107: \"ACK\"\n").unwrap();
        assert_eq!((d.fields[1].start, d.fields[1].end), (106, 106));
        assert_eq!((d.fields[2].start, d.fields[2].end), (107, 107));
    }

    #[test]
    fn 相対指定は直前の終端に続く() {
        // 先頭の +N は 0 始まり
        let d =
            parse("packet\n+16: \"Source Port\"\n+16: \"Destination Port\"\n+1: \"F\"\n").unwrap();
        assert_eq!((d.fields[0].start, d.fields[0].end), (0, 15));
        assert_eq!((d.fields[1].start, d.fields[1].end), (16, 31));
        assert_eq!((d.fields[2].start, d.fields[2].end), (32, 32));
    }

    #[test]
    fn 絶対指定と相対指定を混在できる() {
        let d = parse("packet\n0-15: \"a\"\n+16: \"b\"\n32: \"c\"\n").unwrap();
        assert_eq!((d.fields[1].start, d.fields[1].end), (16, 31));
        assert_eq!((d.fields[2].start, d.fields[2].end), (32, 32));
    }

    #[test]
    fn packet_beta_ヘッダも受理する() {
        assert!(parse("packet-beta\n0-7: \"Type\"\n").is_ok());
    }

    #[test]
    fn frontmatter_の_title_の引用符を剥がす() {
        let d = parse("---\ntitle: \"TCP Packet\"\n---\npacket\n0-15: \"x\"\n").unwrap();
        assert_eq!(d.title.as_deref(), Some("TCP Packet"));
        // 引用符なしはそのまま
        let d = parse("---\ntitle: パケット構造\n---\npacket\n0-15: \"x\"\n").unwrap();
        assert_eq!(d.title.as_deref(), Some("パケット構造"));
    }

    #[test]
    fn 本文の_title_行と裸ラベルも受理する() {
        let d = parse("packet\ntitle UDP Header\n0-15: Source Port\n").unwrap();
        assert_eq!(d.title.as_deref(), Some("UDP Header"));
        assert_eq!(d.fields[0].label, "Source Port");
    }

    #[test]
    fn ビットの隙間はエラー() {
        let e = parse("packet\n0-15: \"a\"\n20-31: \"b\"\n").unwrap_err();
        assert!(e.to_string().contains("16〜19"), "{e}");
    }

    #[test]
    fn ビットの重複はエラー() {
        let e = parse("packet\n0-15: \"a\"\n8-31: \"b\"\n").unwrap_err();
        assert!(e.to_string().contains("重複"), "{e}");
    }

    #[test]
    fn 終端が開始より前はエラー() {
        let e = parse("packet\n15-0: \"a\"\n").unwrap_err();
        assert!(e.to_string().contains("より前"), "{e}");
    }

    #[test]
    fn 非数値とプラスゼロはエラー() {
        assert!(parse("packet\nx-15: \"a\"\n").is_err());
        assert!(parse("packet\n0-y: \"a\"\n").is_err());
        assert!(parse("packet\n+0: \"a\"\n").is_err());
        assert!(parse("packet\n0-15 \"コロン無し\"\n").is_err());
    }

    #[test]
    fn ヘッダ無しとフィールドゼロはエラー() {
        assert!(parse("0-15: \"a\"\n").is_err());
        assert!(parse("packet TB\n0-15: \"a\"\n").is_err());
        assert!(parse("packet\ntitle だけ\n").is_err());
    }

    #[test]
    fn 上限を超えるビットはエラー() {
        assert!(parse("packet\n0-4095: \"限界まで OK\"\n").is_ok());
        assert!(parse("packet\n0-4096: \"上限超え\"\n").is_err());
        assert!(parse("packet\n0-4294967295: \"u32 最大値\"\n").is_err());
    }

    #[test]
    fn config_の_bits_per_row_を拾う() {
        let d = parse(
            "---\ntitle: \"IPv4\"\nconfig:\n  packet:\n    bitsPerRow: 16\n    showBits: false\n---\npacket\n0-15: \"x\"\n",
        )
        .unwrap();
        assert_eq!(d.config.bits_per_row, 16);
        assert!(!d.config.show_bits);
        assert_eq!(d.title.as_deref(), Some("IPv4"));
    }

    #[test]
    fn config_の未知キーと他セクションは無視する() {
        let d = parse(
            "---\nconfig:\n  theme: dark\n  flowchart:\n    curve: basis\n  packet:\n    unknownKey: 1\n    bitWidth: 24\n---\npacket\n0-7: \"x\"\n",
        )
        .unwrap();
        assert_eq!(d.config.bit_width, 24.0);
        assert_eq!(d.config.bits_per_row, 32, "他は既定値のまま");
    }

    #[test]
    fn config_の_padding_x_が_bit_width_以上はエラー() {
        // 単一ビットのブロック幅が bitWidth - paddingX で負になる組み合わせ
        let e = parse(
            "---\nconfig:\n  packet:\n    bitWidth: 1\n    paddingX: 2\n---\npacket\n0: \"x\"\n",
        )
        .unwrap_err();
        assert!(e.to_string().contains("paddingX"), "{e}");
        // 等しい場合も幅 0 になるのでエラー
        assert!(
            parse(
                "---\nconfig:\n  packet:\n    bitWidth: 8\n    paddingX: 8\n---\npacket\n0: \"x\"\n"
            )
            .is_err()
        );
        // 小さければ受理（キーの指定順にも依存しない）
        assert!(
            parse(
                "---\nconfig:\n  packet:\n    paddingX: 2\n    bitWidth: 8\n---\npacket\n0: \"x\"\n"
            )
            .is_ok()
        );
    }

    #[test]
    fn config_の不正値はエラー() {
        assert!(
            parse("---\nconfig:\n  packet:\n    bitsPerRow: 0\n---\npacket\n0: \"x\"\n").is_err()
        );
        assert!(
            parse("---\nconfig:\n  packet:\n    bitsPerRow: abc\n---\npacket\n0: \"x\"\n").is_err()
        );
        assert!(
            parse("---\nconfig:\n  packet:\n    showBits: yes\n---\npacket\n0: \"x\"\n").is_err()
        );
        assert!(
            parse("---\nconfig:\n  packet:\n    bitWidth: -1\n---\npacket\n0: \"x\"\n").is_err()
        );
    }
}
