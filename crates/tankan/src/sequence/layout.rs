//! sequence 図の定規レイアウト。
//!
//! - X: 参加者の箱幅とラベル幅から隣接ギャップの制約を集め、左から累積
//! - Y: 単一カーソルをイベント順に進める
//!
//! 出力は座標確定済みのプリミティブ列 [`Layout`]（SVG 生成とテストを分離するため）。
//! 定数は mermaid の既定値に準拠する。

use crate::Options;
use crate::common::text::max_width;
use crate::sequence::model::{BlockKind, Event, HeadKind, LineKind, NotePos, SequenceDiagram};

// mermaid 既定値準拠の定数
const DIAGRAM_MARGIN_X: f32 = 50.0;
const DIAGRAM_MARGIN_Y: f32 = 10.0;
const ACTOR_MARGIN: f32 = 50.0;
const ACTOR_MIN_W: f32 = 150.0;
const ACTOR_PAD_Y: f32 = 12.0;
const TEXT_PAD: f32 = 10.0;
const MESSAGE_GAP: f32 = 25.0;
const ACTIVATION_W: f32 = 10.0;
const ACTIVATION_NEST_OFFSET: f32 = 3.0;
const NOTE_MARGIN: f32 = 10.0;
const NOTE_PAD: f32 = 8.0;
const NOTE_OVERLAP: f32 = 25.0;
const BOX_MARGIN: f32 = 10.0;
const FRAME_LABEL_H: f32 = 20.0;
const FRAME_PAD_BASE: f32 = 10.0;
const FRAME_PAD_STEP: f32 = 6.0;
const SELF_MSG_W: f32 = 40.0;
const SELF_MSG_H: f32 = 20.0;
const AUTONUMBER_PAD: f32 = 24.0;

pub(crate) struct Layout {
    pub width: f32,
    pub height: f32,
    pub line_h: f32,
    pub title: Option<TextAt>,
    pub actors: Vec<ActorBox>,
    pub actor_top_y: f32,
    pub actor_h: f32,
    /// 下端ミラーボックスの上辺 y（mermaid の mirrorActors 相当）
    pub mirror_y: f32,
    /// (x, y1, y2)
    pub lifelines: Vec<(f32, f32, f32)>,
    pub group_boxes: Vec<GroupBox>,
    pub rect_bgs: Vec<RectBg>,
    pub activations: Vec<ActBar>,
    pub frames: Vec<Frame>,
    pub messages: Vec<Msg>,
    pub notes: Vec<NoteBox>,
}

pub(crate) struct TextAt {
    /// text-anchor="middle" の中心 x
    pub x: f32,
    /// 1 行目のベースライン y
    pub y: f32,
    pub lines: Vec<String>,
}

pub(crate) struct ActorBox {
    pub cx: f32,
    pub w: f32,
    pub lines: Vec<String>,
    pub is_actor: bool,
}

pub(crate) struct GroupBox {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub label: Vec<String>,
    pub color: Option<String>,
}

pub(crate) struct RectBg {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub color: String,
}

pub(crate) struct ActBar {
    pub x: f32,
    pub y1: f32,
    pub y2: f32,
}

pub(crate) struct Frame {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    /// 五角形内のキーワード（"loop" 等）
    pub kind: &'static str,
    /// 条件ラベル
    pub label: Vec<String>,
    /// (y, ラベル) — else/and/option の区切り
    pub separators: Vec<(f32, Vec<String>)>,
}

pub(crate) struct Msg {
    pub x1: f32,
    pub x2: f32,
    /// 矢印線の y
    pub y: f32,
    pub line: LineKind,
    pub head: HeadKind,
    pub text: Option<TextAt>,
    pub self_msg: bool,
    pub number: Option<u32>,
}

pub(crate) struct NoteBox {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub lines: Vec<String>,
}

/// レイアウト中のブロックフレーム
struct OpenFrame {
    kind: BlockKind,
    label: Vec<String>,
    top: f32,
    min_i: usize,
    max_i: usize,
    touched: bool,
    separators: Vec<(f32, Vec<String>)>,
    /// 子フレームの張り出しを包含するためのパディング
    pad: f32,
}

pub(crate) fn layout(diagram: &SequenceDiagram, options: &Options) -> Layout {
    let fs = options.font_size;
    let line_h = fs * 1.4;
    let n = diagram.participants.len();

    // ---- X パス ----
    let widths: Vec<f32> = diagram
        .participants
        .iter()
        .map(|p| (max_width(&p.display, fs) + 2.0 * TEXT_PAD).max(ACTOR_MIN_W))
        .collect();

    let mut gaps: Vec<f32> = (0..n.saturating_sub(1))
        .map(|i| (widths[i] + widths[i + 1]) / 2.0 + ACTOR_MARGIN)
        .collect();
    let mut left_extend: f32 = 0.0;
    let mut right_extend: f32 = 0.0;
    // (lo, hi, 必要な中心間距離) — hi-lo >= 2 の制約
    let mut span_constraints: Vec<(usize, usize, f32)> = Vec::new();

    let autonumber_used = diagram
        .events
        .iter()
        .any(|e| matches!(e, Event::AutonumberOn { .. }));
    let num_pad = if autonumber_used { AUTONUMBER_PAD } else { 0.0 };

    for event in &diagram.events {
        match event {
            Event::Message { from, to, text, .. } => {
                let label_w = max_width(text, fs);
                if from == to {
                    let req = SELF_MSG_W + label_w + 2.0 * TEXT_PAD + num_pad;
                    let i = *from;
                    if i + 1 < n {
                        gaps[i] = gaps[i].max(req + widths[i + 1] / 2.0);
                    } else {
                        right_extend = right_extend.max(req - widths[i] / 2.0);
                    }
                } else {
                    let (lo, hi) = (*from.min(to), *from.max(to));
                    let req = label_w + 2.0 * TEXT_PAD + num_pad;
                    if hi - lo == 1 {
                        gaps[lo] = gaps[lo].max(req);
                    } else {
                        span_constraints.push((lo, hi, req));
                    }
                }
            }
            Event::Note { pos, a, b, text } => {
                let note_w = max_width(text, fs) + 2.0 * NOTE_PAD;
                match (pos, b) {
                    (NotePos::Over, Some(b)) => {
                        let (lo, hi) = (*a.min(b), *a.max(b));
                        let req = note_w - 2.0 * NOTE_OVERLAP;
                        if req > 0.0 {
                            if hi - lo == 1 {
                                gaps[lo] = gaps[lo].max(req);
                            } else {
                                span_constraints.push((lo, hi, req));
                            }
                        }
                    }
                    (NotePos::Over, None) => {
                        let half = note_w / 2.0;
                        let i = *a;
                        if i > 0 {
                            gaps[i - 1] = gaps[i - 1].max(half + widths[i - 1] / 2.0 + 5.0);
                        } else {
                            left_extend = left_extend.max(half - widths[0] / 2.0);
                        }
                        if i + 1 < n {
                            gaps[i] = gaps[i].max(half + widths[i + 1] / 2.0 + 5.0);
                        } else {
                            right_extend = right_extend.max(half - widths[i] / 2.0);
                        }
                    }
                    (NotePos::LeftOf, _) => {
                        let i = *a;
                        let req = note_w + NOTE_MARGIN;
                        if i > 0 {
                            gaps[i - 1] = gaps[i - 1].max(req + widths[i - 1] / 2.0);
                        } else {
                            left_extend = left_extend.max(req - widths[0] / 2.0);
                        }
                    }
                    (NotePos::RightOf, _) => {
                        let i = *a;
                        let req = note_w + NOTE_MARGIN;
                        if i + 1 < n {
                            gaps[i] = gaps[i].max(req + widths[i + 1] / 2.0);
                        } else {
                            right_extend = right_extend.max(req - widths[i] / 2.0);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    // span >= 2 の制約: 区間合計が足りなければ均等に加算（狭い区間から処理）
    span_constraints.sort_by_key(|&(lo, hi, _)| hi - lo);
    for (lo, hi, req) in span_constraints {
        let sum: f32 = gaps[lo..hi].iter().sum();
        if sum < req {
            let add = (req - sum) / (hi - lo) as f32;
            for gap in &mut gaps[lo..hi] {
                *gap += add;
            }
        }
    }

    let mut xs: Vec<f32> = Vec::with_capacity(n);
    if n > 0 {
        let mut x = DIAGRAM_MARGIN_X + left_extend.max(0.0) + widths[0] / 2.0;
        xs.push(x);
        for gap in &gaps {
            x += gap;
            xs.push(x);
        }
    }
    let width = match n {
        0 => 2.0 * DIAGRAM_MARGIN_X,
        _ => xs[n - 1] + widths[n - 1] / 2.0 + right_extend.max(0.0) + DIAGRAM_MARGIN_X,
    };

    // ---- Y パス ----
    let mut cur_y = DIAGRAM_MARGIN_Y;

    let title = diagram.title.as_ref().map(|lines| {
        let t = TextAt {
            x: width / 2.0,
            y: cur_y + line_h - 4.0,
            lines: lines.clone(),
        };
        cur_y += lines.len() as f32 * line_h + 8.0;
        t
    });

    // box バンド（参加者ボックスの上にラベル分の余白）
    let has_boxes = !diagram.boxes.is_empty();
    let box_top = cur_y;
    if has_boxes {
        cur_y += FRAME_LABEL_H + 6.0;
    }

    let actor_top_y = cur_y;
    let actor_h = diagram
        .participants
        .iter()
        .map(|p| p.display.len() as f32 * line_h + 2.0 * ACTOR_PAD_Y)
        .fold(38.0, f32::max);
    cur_y += actor_h + 20.0;

    let mut messages = Vec::new();
    let mut notes = Vec::new();
    let mut frames = Vec::new();
    let mut rect_bgs = Vec::new();
    let mut activations = Vec::new();
    let mut frame_stack: Vec<OpenFrame> = Vec::new();
    // 参加者ごとの activation 開始 y スタック
    let mut act_stacks: Vec<Vec<f32>> = vec![Vec::new(); n];
    let mut counter: Option<(u32, u32)> = None; // (次の番号, step)

    let touch = |stack: &mut Vec<OpenFrame>, i: usize| {
        for frame in stack.iter_mut() {
            if frame.touched {
                frame.min_i = frame.min_i.min(i);
                frame.max_i = frame.max_i.max(i);
            } else {
                frame.min_i = i;
                frame.max_i = i;
                frame.touched = true;
            }
        }
    };

    for event in &diagram.events {
        match event {
            Event::Message {
                from,
                to,
                line,
                head,
                text,
                activate_to,
                deactivate_from,
            } => {
                touch(&mut frame_stack, *from);
                touch(&mut frame_stack, *to);
                let number = counter.map(|(next, step)| {
                    counter = Some((next + step, step));
                    next
                });

                if from == to {
                    // 自己メッセージ: C 字カーブ。ラベルはカーブの右
                    let x = xs[*from];
                    let start_y = cur_y + 4.0;
                    let text_block = (!text.is_empty() && !text[0].is_empty()).then(|| TextAt {
                        x: x + SELF_MSG_W + 6.0,
                        y: start_y + line_h / 2.0,
                        lines: text.clone(),
                    });
                    let lines_h = text.len() as f32 * line_h;
                    messages.push(Msg {
                        x1: x,
                        x2: x,
                        y: start_y,
                        line: *line,
                        head: *head,
                        text: text_block,
                        self_msg: true,
                        number,
                    });
                    if *activate_to {
                        act_stacks[*to].push(start_y + SELF_MSG_H);
                    }
                    if *deactivate_from {
                        if let Some(y1) = act_stacks[*from].pop() {
                            activations.push(bar(xs[*from], act_stacks[*from].len(), y1, start_y));
                        }
                    }
                    cur_y += (lines_h.max(SELF_MSG_H) + 4.0) + MESSAGE_GAP;
                } else {
                    let lines_h = if text.is_empty() || text[0].is_empty() {
                        0.0
                    } else {
                        text.len() as f32 * line_h
                    };
                    let arrow_y = cur_y + lines_h + 6.0;
                    let text_block = (lines_h > 0.0).then(|| TextAt {
                        x: (xs[*from] + xs[*to]) / 2.0,
                        y: cur_y + line_h - 4.0,
                        lines: text.clone(),
                    });
                    if *activate_to {
                        act_stacks[*to].push(arrow_y);
                    }
                    if *deactivate_from {
                        if let Some(y1) = act_stacks[*from].pop() {
                            activations.push(bar(xs[*from], act_stacks[*from].len(), y1, arrow_y));
                        }
                    }
                    messages.push(Msg {
                        x1: xs[*from],
                        x2: xs[*to],
                        y: arrow_y,
                        line: *line,
                        head: *head,
                        text: text_block,
                        self_msg: false,
                        number,
                    });
                    cur_y = arrow_y + MESSAGE_GAP;
                }
            }
            Event::Note { pos, a, b, text } => {
                touch(&mut frame_stack, *a);
                if let Some(b) = b {
                    touch(&mut frame_stack, *b);
                }
                let note_w = max_width(text, fs) + 2.0 * NOTE_PAD;
                let h = text.len() as f32 * line_h + 2.0 * NOTE_PAD;
                let (x, w) = match (pos, b) {
                    (NotePos::LeftOf, _) => (xs[*a] - NOTE_MARGIN - note_w, note_w),
                    (NotePos::RightOf, _) => (xs[*a] + NOTE_MARGIN, note_w),
                    (NotePos::Over, None) => (xs[*a] - note_w / 2.0, note_w),
                    (NotePos::Over, Some(b)) => {
                        let (lo, hi) = (xs[*a.min(b)], xs[*a.max(b)]);
                        let w = (hi - lo + 2.0 * NOTE_OVERLAP).max(note_w);
                        ((lo + hi) / 2.0 - w / 2.0, w)
                    }
                };
                notes.push(NoteBox {
                    x,
                    y: cur_y,
                    w,
                    h,
                    lines: text.clone(),
                });
                cur_y += h + NOTE_MARGIN;
            }
            Event::Activate(id) => {
                act_stacks[*id].push(cur_y);
                touch(&mut frame_stack, *id);
            }
            Event::Deactivate(id) => {
                if let Some(y1) = act_stacks[*id].pop() {
                    activations.push(bar(xs[*id], act_stacks[*id].len(), y1, cur_y));
                }
            }
            Event::BlockBegin { kind, label } => {
                frame_stack.push(OpenFrame {
                    kind: kind.clone(),
                    label: label.clone(),
                    top: cur_y,
                    min_i: 0,
                    max_i: 0,
                    touched: false,
                    separators: Vec::new(),
                    pad: FRAME_PAD_BASE,
                });
                cur_y += FRAME_LABEL_H + 8.0;
            }
            Event::BlockSeparator { label } => {
                if let Some(frame) = frame_stack.last_mut() {
                    frame.separators.push((cur_y, label.clone()));
                }
                cur_y += FRAME_LABEL_H + 4.0;
            }
            Event::BlockEnd => {
                if let Some(open) = frame_stack.pop() {
                    let bottom = cur_y + 4.0;
                    cur_y = bottom + BOX_MARGIN;
                    // ブロック内で誰も登場しなければ全参加者にかける
                    let (min_i, max_i) = if open.touched {
                        (open.min_i, open.max_i)
                    } else {
                        (0, n.saturating_sub(1))
                    };
                    // 親フレームへ範囲とパディングを伝播（包含を保証）
                    if let Some(parent) = frame_stack.last_mut() {
                        if open.touched {
                            if parent.touched {
                                parent.min_i = parent.min_i.min(min_i);
                                parent.max_i = parent.max_i.max(max_i);
                            } else {
                                parent.min_i = min_i;
                                parent.max_i = max_i;
                                parent.touched = true;
                            }
                        }
                        parent.pad = parent.pad.max(open.pad + FRAME_PAD_STEP);
                    }
                    if n == 0 {
                        continue;
                    }
                    let x1 = xs[min_i] - open.pad - ACTIVATION_W;
                    let x2 = xs[max_i] + open.pad + ACTIVATION_W;
                    match open.kind {
                        BlockKind::Rect(color) => rect_bgs.push(RectBg {
                            x: x1,
                            y: open.top,
                            w: x2 - x1,
                            h: bottom - open.top,
                            color,
                        }),
                        kind => frames.push(Frame {
                            x: x1,
                            y: open.top,
                            w: x2 - x1,
                            h: bottom - open.top,
                            kind: kind.label(),
                            label: open.label,
                            separators: open.separators,
                        }),
                    }
                }
            }
            Event::AutonumberOn { start, step } => counter = Some((*start, *step)),
            Event::AutonumberOff => counter = None,
        }
    }

    // 未クローズの activation はライフライン下端まで
    let lifeline_bottom = cur_y + 6.0;
    for (i, stack) in act_stacks.iter().enumerate() {
        for (depth, &y1) in stack.iter().enumerate() {
            activations.push(bar(xs[i], depth, y1, lifeline_bottom));
        }
    }

    let mirror_y = lifeline_bottom;
    let mut height = mirror_y + actor_h + DIAGRAM_MARGIN_Y;

    // box（参加者グルーピング）
    let mut group_boxes = Vec::new();
    for pbox in &diagram.boxes {
        if pbox.members.is_empty() {
            continue;
        }
        let lo = pbox.members.iter().copied().min().unwrap_or(0);
        let hi = pbox.members.iter().copied().max().unwrap_or(0);
        let x1 = xs[lo] - widths[lo] / 2.0 - BOX_MARGIN;
        let x2 = xs[hi] + widths[hi] / 2.0 + BOX_MARGIN;
        group_boxes.push(GroupBox {
            x: x1,
            y: box_top,
            w: x2 - x1,
            h: mirror_y + actor_h + BOX_MARGIN - box_top,
            label: pbox.label.clone(),
            color: pbox.color.clone(),
        });
    }
    if has_boxes {
        height += BOX_MARGIN;
    }

    let actors: Vec<ActorBox> = diagram
        .participants
        .iter()
        .enumerate()
        .map(|(i, p)| ActorBox {
            cx: xs[i],
            w: widths[i],
            lines: p.display.clone(),
            is_actor: p.is_actor,
        })
        .collect();

    let lifelines: Vec<(f32, f32, f32)> = xs
        .iter()
        .map(|&x| (x, actor_top_y + actor_h, mirror_y))
        .collect();

    Layout {
        width,
        height,
        line_h,
        title,
        actors,
        actor_top_y,
        actor_h,
        mirror_y,
        lifelines,
        group_boxes,
        rect_bgs,
        activations,
        frames,
        messages,
        notes,
    }
}

fn bar(x: f32, depth: usize, y1: f32, y2: f32) -> ActBar {
    ActBar {
        x: x - ACTIVATION_W / 2.0 + depth as f32 * ACTIVATION_NEST_OFFSET,
        y1,
        y2,
    }
}

#[cfg(test)]
mod tests {
    use super::layout;
    use crate::Options;
    use crate::sequence::parser::parse;

    fn lay(body: &str) -> super::Layout {
        let d = parse(&format!("sequenceDiagram\n{body}")).unwrap();
        layout(&d, &Options::default())
    }

    #[test]
    fn 二参加者一メッセージの基本座標() {
        let l = lay("A->>B: hi");
        assert_eq!(l.actors.len(), 2);
        // A は左マージン + 箱半分
        assert_eq!(l.actors[0].cx, 50.0 + 75.0);
        // 中心間距離 = 箱幅平均 + マージン = 150 + 50
        assert_eq!(l.actors[1].cx - l.actors[0].cx, 200.0);
        assert_eq!(l.messages.len(), 1);
        let m = &l.messages[0];
        assert_eq!(m.x1, l.actors[0].cx);
        assert_eq!(m.x2, l.actors[1].cx);
        assert!(m.y > l.actor_top_y + l.actor_h);
        assert!(l.mirror_y > m.y);
        assert!(l.width > l.actors[1].cx + 75.0);
    }

    #[test]
    fn 長いラベルでギャップが広がる() {
        let short = lay("A->>B: hi");
        let long = lay("A->>B: とてもとてもとてもとてもとてもとても長いメッセージラベルです");
        assert!(
            long.actors[1].cx - long.actors[0].cx > short.actors[1].cx - short.actors[0].cx,
            "long={} short={}",
            long.actors[1].cx - long.actors[0].cx,
            short.actors[1].cx - short.actors[0].cx
        );
    }

    #[test]
    fn スパンをまたぐ制約は区間合計に効く() {
        let l = lay(
            "A->>B: x\nB->>C: y\nA->>C: とてもとてもとてもとてもとてもとてもとても長いラベルをここに置きます",
        );
        let total = l.actors[2].cx - l.actors[0].cx;
        assert!(total > 400.0, "total={total}"); // 既定 2 区間 = 400 より広がる
    }

    #[test]
    fn ネストしたフレームは親が子を包含する() {
        let l = lay("loop 外側\nalt 内側\nA->>B: x\nelse その他\nB->>A: y\nend\nend");
        assert_eq!(l.frames.len(), 2);
        // frames は pop 順（内側が先）
        let inner = &l.frames[0];
        let outer = &l.frames[1];
        assert!(outer.x < inner.x);
        assert!(outer.x + outer.w > inner.x + inner.w);
        assert!(outer.y < inner.y);
        assert!(outer.y + outer.h > inner.y + inner.h);
        assert_eq!(inner.separators.len(), 1);
    }

    #[test]
    fn 自己メッセージは右に張り出す() {
        let l = lay("A->>A: 自分自身への長いメッセージ");
        assert!(l.messages[0].self_msg);
        // 全体幅が箱の右端より十分広い（ラベル分）
        assert!(l.width > l.actors[0].cx + 75.0 + 40.0);
    }

    #[test]
    fn activation_バーが積まれる() {
        let l = lay("A->>+B: req\nB-->>-A: res");
        assert_eq!(l.activations.len(), 1);
        let bar = &l.activations[0];
        assert!(bar.y2 > bar.y1);
        // B のライフライン上（中心 ± 5px）
        assert!((bar.x + 5.0 - l.actors[1].cx).abs() < 0.01);
    }

    #[test]
    fn 未クローズ_activation_は下端まで伸びる() {
        let l = lay("activate A\nA->>B: x");
        assert_eq!(l.activations.len(), 1);
        assert_eq!(l.activations[0].y2, l.mirror_y);
    }

    #[test]
    fn autonumber_で番号が振られる() {
        let l = lay("autonumber\nA->>B: x\nB->>A: y\nautonumber off\nA->>B: z");
        let nums: Vec<Option<u32>> = l.messages.iter().map(|m| m.number).collect();
        assert_eq!(nums, [Some(1), Some(2), None]);
    }
}
