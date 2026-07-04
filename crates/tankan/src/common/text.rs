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
}
