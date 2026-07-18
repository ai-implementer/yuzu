//! ブラウザなしのテキスト計測と、テキスト処理ユーティリティ。
//!
//! 計測は近似: ASCII はメトリクステーブル、非 ASCII は East Asian Width。
//! 閲覧側はシステムフォントで描画されるため厳密一致は原理的に不可能で、
//! 「余白側に倒した安全な近似」を方針とする（誤差はパディングで吸収）。

use unicode_width::UnicodeWidthChar;

/// ASCII 印字文字（0x20〜0x7E）の幅テーブル。単位は 1/1000 em。
/// Helvetica/Arial 系の代表的なメトリクスに基づく近似値
#[rustfmt::skip]
const ASCII_WIDTHS: [u16; 95] = [
    // ' '  !    "    #    $    %    &    '    (    )    *    +    ,    -    .    /
    278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278, 278,
    // 0    1    2    3    4    5    6    7    8    9    :    ;    <    =    >    ?
    556, 556, 556, 556, 556, 556, 556, 556, 556, 556, 278, 278, 584, 584, 584, 556,
    // @    A    B    C    D    E    F    G    H    I    J    K    L    M    N    O
    1015, 667, 667, 722, 722, 667, 611, 778, 722, 278, 500, 667, 556, 833, 722, 778,
    // P    Q    R    S    T    U    V    W    X    Y    Z    [    \    ]    ^    _
    667, 778, 722, 667, 611, 722, 667, 944, 667, 667, 611, 278, 278, 278, 469, 556,
    // `    a    b    c    d    e    f    g    h    i    j    k    l    m    n    o
    333, 556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500, 222, 833, 556, 556,
    // p    q    r    s    t    u    v    w    x    y    z    {    |    }    ~
    556, 556, 333, 500, 278, 556, 500, 722, 500, 500, 500, 334, 260, 334, 584,
];

/// 1 行のテキスト幅（px）を近似計算する
pub(crate) fn text_width(text: &str, font_size: f32) -> f32 {
    let em: f32 = text
        .chars()
        .map(|c| {
            if let Some(i) = (c as u32).checked_sub(0x20) {
                if (i as usize) < ASCII_WIDTHS.len() {
                    return f32::from(ASCII_WIDTHS[i as usize]) / 1000.0;
                }
            }
            // 非 ASCII は East Asian Width（cjk = Ambiguous を全角扱い＝安全側）
            match UnicodeWidthChar::width_cjk(c) {
                Some(2) => 1.0,
                Some(0) | None => 0.0,
                _ => 0.6,
            }
        })
        .sum();
    em * font_size
}

/// 複数行のうち最大の幅
pub(crate) fn max_width(lines: &[String], font_size: f32) -> f32 {
    lines
        .iter()
        .map(|l| text_width(l, font_size))
        .fold(0.0, f32::max)
}

/// `<br/>`（`<br>`, `<br />` も）でテキストを行に分割する
pub(crate) fn split_br_lines(text: &str) -> Vec<String> {
    let normalized = text
        .replace("<br/>", "\n")
        .replace("<br />", "\n")
        .replace("<br>", "\n");
    normalized
        .split('\n')
        .map(|s| s.trim().to_string())
        .collect()
}

/// `max_w`（px）に収まるよう貪欲に折り返す。`<br/>` の明示分割が最優先。
/// ASCII は単語境界を優先し、単語単独で収まらない長語と CJK は文字単位で折る。
/// 1 行に最低 1 単位は必ず置く（max_w が極小でも停止する）。
/// 行頭禁則は最小限: 次の単位が 1 文字の閉じ記号なら予算超過を許して現在行に付ける
/// （幅計測は元々近似で、誤差はパディング側で吸収する方針のため）
pub(crate) fn wrap_text(text: &str, font_size: f32, max_w: f32) -> Vec<String> {
    let mut out = Vec::new();
    for line in split_br_lines(text) {
        wrap_line(&line, font_size, max_w, &mut out);
    }
    out
}

/// 折返しの 1 単位: 連続する ASCII 非空白 = 1 単位（単語）、それ以外は 1 文字 = 1 単位。
/// ASCII 空白は単語間の区切りとして保持する（行頭に来たら捨てる）
fn wrap_units(line: &str) -> Vec<String> {
    let mut units: Vec<String> = Vec::new();
    let mut word = String::new();
    for c in line.chars() {
        if c.is_ascii() && !c.is_ascii_whitespace() {
            word.push(c);
            continue;
        }
        if !word.is_empty() {
            units.push(std::mem::take(&mut word));
        }
        units.push(c.to_string());
    }
    if !word.is_empty() {
        units.push(word);
    }
    units
}

/// 行末に付けてよい 1 文字の閉じ記号（最小限の行頭禁則）
fn is_closing_punct(unit: &str) -> bool {
    let mut chars = unit.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) => "、。，．）」』】〉》]}!?！？…ー".contains(c),
        _ => false,
    }
}

fn wrap_line(line: &str, font_size: f32, max_w: f32, out: &mut Vec<String>) {
    let units = wrap_units(line);
    let mut cur = String::new();
    let push_unit = |cur: &mut String, unit: &str, out: &mut Vec<String>| {
        let candidate = format!("{cur}{unit}");
        if cur.is_empty() || text_width(&candidate, font_size) <= max_w || is_closing_punct(unit) {
            *cur = candidate;
            return;
        }
        // 行末の空白は落として確定（"hello " → "hello"）
        out.push(cur.trim_end().to_string());
        cur.clear();
        // 行頭の ASCII 空白は捨てる（折返し位置の空白を持ち越さない）
        if !unit.chars().all(|c| c.is_ascii_whitespace()) {
            *cur = unit.to_string();
        }
    };
    for unit in units {
        // 単位単独で max_w を超える長い ASCII 語は文字単位に割る
        if unit.chars().count() > 1 && text_width(&unit, font_size) > max_w {
            for c in unit.chars() {
                push_unit(&mut cur, &c.to_string(), out);
            }
            continue;
        }
        push_unit(&mut cur, &unit, out);
    }
    // 空行も 1 行として保持する（<br/><br/> の空行を潰さない）
    out.push(cur.trim_end().to_string());
}

/// mermaid のエンティティ表記 `#十進数;` を文字にデコードする（例: `#35;` → `#`）
pub(crate) fn decode_entities(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(pos) = rest.find('#') {
        out.push_str(&rest[..pos]);
        let after = &rest[pos + 1..];
        if let Some(end) = after.find(';') {
            if let Ok(code) = after[..end].parse::<u32>() {
                if let Some(c) = char::from_u32(code) {
                    out.push(c);
                    rest = &after[end + 1..];
                    continue;
                }
            }
        }
        out.push('#');
        rest = after;
    }
    out.push_str(rest);
    out
}

/// XML エスケープ（属性値・テキストノード共用の 5 種）
pub(crate) fn escape_xml(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_と_cjk_の幅() {
        // 全角 5 文字 = 5em
        assert_eq!(text_width("こんにちは", 14.0), 5.0 * 14.0);
        // ASCII はテーブル値（"iW" = 222 + 944 = 1166/1000 em）
        let w = text_width("iW", 1000.0);
        assert!((w - 1166.0).abs() < 0.01, "w={w}");
        // 混在
        assert!(text_width("柚子yuzu", 14.0) > text_width("yuzu", 14.0));
    }

    #[test]
    fn br_の_3_表記で分割できる() {
        assert_eq!(split_br_lines("a<br/>b<br>c<br />d"), ["a", "b", "c", "d"]);
        assert_eq!(split_br_lines("単一行"), ["単一行"]);
    }

    #[test]
    fn エンティティのデコード() {
        assert_eq!(decode_entities("A#35;1"), "A#1");
        assert_eq!(decode_entities("#59;セミコロン"), ";セミコロン");
        // 不正な形はそのまま
        assert_eq!(decode_entities("#xyz;"), "#xyz;");
        assert_eq!(decode_entities("100#"), "100#");
    }

    #[test]
    fn xml_エスケープ() {
        assert_eq!(escape_xml(r#"a<b>&"c'"#), "a&lt;b&gt;&amp;&quot;c&apos;");
    }

    #[test]
    fn wrap_は_ascii_の単語境界を優先する() {
        // "hello world again" を狭い幅で折ると単語ごとに割れる
        let lines = wrap_text("hello world again", 10.0, 40.0);
        assert_eq!(lines, ["hello", "world", "again"]);
        // 行頭に空白は持ち越されない
        assert!(lines.iter().all(|l| !l.starts_with(' ')), "{lines:?}");
    }

    #[test]
    fn wrap_は_cjk_を文字単位で折る() {
        // 全角 1 文字 = font_size px。幅 3 文字分で 7 文字 → 3+3+1
        let lines = wrap_text("あいうえおかき", 10.0, 30.0);
        assert_eq!(lines, ["あいう", "えおか", "き"]);
    }

    #[test]
    fn wrap_は_br_の明示分割が優先される() {
        let lines = wrap_text("短い<br/>これはとても長い行です", 10.0, 50.0);
        assert_eq!(lines[0], "短い", "{lines:?}");
        assert!(lines.len() > 2, "後半は幅で折られる: {lines:?}");
    }

    #[test]
    fn wrap_は長い_ascii_語を文字単位に割る() {
        let lines = wrap_text("supercalifragilistic", 10.0, 40.0);
        assert!(lines.len() > 1, "{lines:?}");
        assert_eq!(lines.concat(), "supercalifragilistic", "文字は失わない");
    }

    #[test]
    fn wrap_は空文字列と極小幅でも停止する() {
        assert_eq!(wrap_text("", 10.0, 100.0), [""]);
        // max_w が 1 文字分未満でも 1 行 1 単位で必ず前進する
        let lines = wrap_text("あいう", 10.0, 1.0);
        assert_eq!(lines, ["あ", "い", "う"]);
    }

    #[test]
    fn wrap_は行頭の閉じ記号を前行に付ける() {
        // 「。」が行頭に来る位置で折れるケース → 予算超過を許して前行末尾へ
        let lines = wrap_text("あいう。えお", 10.0, 30.0);
        assert_eq!(lines, ["あいう。", "えお"]);
    }
}
