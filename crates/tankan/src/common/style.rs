//! 図種をまたぐスタイル指定（`classDef` / `class`(`cssClass`) / `:::` / `style`）の
//! 共通実装。パース・マージ・解決（`StyleCollector`）と、SVG のインライン `style=""`
//! 属性・ラベル文字色の自動選択を 1 箇所に集約する。
//!
//! テーマ非追従が正: ユーザ指定色は SVG の `style=""` 属性に直接埋める
//! （`<style>` 追記方式は同一ページの複数 SVG でルールが衝突するため不可）。

use std::collections::HashMap;

use crate::common::text::escape_xml;

/// 箱（ノード / エンティティ / クラスボックス）に適用するインラインスタイル。
/// 未指定プロパティは None。
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Style {
    pub fill: Option<String>,
    pub stroke: Option<String>,
    pub stroke_width: Option<String>,
    pub stroke_dasharray: Option<String>,
    /// ラベル文字色（SVG text の fill）
    pub color: Option<String>,
}

/// `fill:#f9f, stroke:#333, stroke-dasharray: 5 5` 形のスタイル宣言を解析する。
/// カンマ区切り・各要素は最初の `:` で k:v 分割・両端 trim。未知プロパティ・
/// `:` 無し・空値の要素は黙って無視する
pub(crate) fn parse_props(s: &str) -> Style {
    let mut style = Style::default();
    for item in s.split(',') {
        let Some((key, value)) = item.split_once(':') else {
            continue;
        };
        let value = value.trim();
        if value.is_empty() {
            continue;
        }
        let value = Some(value.to_string());
        match key.trim() {
            "fill" => style.fill = value,
            "stroke" => style.stroke = value,
            "stroke-width" => style.stroke_width = value,
            "stroke-dasharray" => style.stroke_dasharray = value,
            "color" => style.color = value,
            _ => {}
        }
    }
    style
}

/// src の Some プロパティだけを dst に上書きする（プロパティ単位の後勝ち）
pub(crate) fn merge(dst: &mut Style, src: &Style) {
    if src.fill.is_some() {
        dst.fill = src.fill.clone();
    }
    if src.stroke.is_some() {
        dst.stroke = src.stroke.clone();
    }
    if src.stroke_width.is_some() {
        dst.stroke_width = src.stroke_width.clone();
    }
    if src.stroke_dasharray.is_some() {
        dst.stroke_dasharray = src.stroke_dasharray.clone();
    }
    if src.color.is_some() {
        dst.color = src.color.clone();
    }
}

/// スタイル指定文（`classDef` / `class`(`cssClass`) / `:::` / `style`）を蓄積し、
/// 箱の名前ごとに解決する。名前 → index の逆引きは各図種側で回す。
#[derive(Debug, Default)]
pub(crate) struct StyleCollector {
    /// classDef で定義したクラス（名前 → スタイル。同名再定義はプロパティ単位後勝ち）
    class_defs: HashMap<String, Style>,
    /// 箱名 → 適用クラス名列（`:::` と class / cssClass 文を出現順に蓄積）
    node_classes: HashMap<String, Vec<String>>,
    /// 箱名 → style 文のスタイル（プロパティ単位後勝ち）
    node_styles: HashMap<String, Style>,
}

impl StyleCollector {
    /// `classDef name1,name2 fill:#f9f,...`（`default` = 全箱の既定）
    pub fn class_def(&mut self, rest: &str) {
        let (names, props) = split_first_ws(rest);
        let style = parse_props(props);
        for name in names.split(',') {
            let name = name.trim();
            if name.is_empty() {
                continue;
            }
            let entry = self.class_defs.entry(name.to_string()).or_default();
            merge(entry, &style);
        }
    }

    /// `class id1,id2 className` / `cssClass id1,id2 className`（適用先は箱のみ）
    pub fn apply_class(&mut self, rest: &str) {
        let (ids, class_name) = split_first_ws(rest);
        let class_name = class_name.trim();
        if class_name.is_empty() {
            return;
        }
        for id in ids.split(',') {
            let id = id.trim();
            if id.is_empty() {
                continue;
            }
            self.add_inline(id, class_name);
        }
    }

    /// `style id1,id2 fill:#bbf,...`（プロパティ単位後勝ち）
    pub fn apply_style(&mut self, rest: &str) {
        let (ids, props) = split_first_ws(rest);
        let style = parse_props(props);
        for id in ids.split(',') {
            let id = id.trim();
            if id.is_empty() {
                continue;
            }
            let entry = self.node_styles.entry(id.to_string()).or_default();
            merge(entry, &style);
        }
    }

    /// インライン `id:::className` 1 件を登録する
    pub fn add_inline(&mut self, id: &str, class_name: &str) {
        self.node_classes
            .entry(id.to_string())
            .or_default()
            .push(class_name.to_string());
    }

    /// スタイル指定が 1 つもなければ true（フルビルドとバイト一致を保つ判定に使う）
    pub fn is_empty(&self) -> bool {
        self.class_defs.is_empty() && self.node_classes.is_empty() && self.node_styles.is_empty()
    }

    /// 1 つの箱へ default → クラス列 → style 文の順にプロパティ単位でマージする。
    /// 1 つでも値があれば Some（未定義クラスはスキップ）
    pub fn resolve(&self, name: &str) -> Option<Style> {
        let mut style = Style::default();
        if let Some(def) = self.class_defs.get("default") {
            merge(&mut style, def);
        }
        if let Some(classes) = self.node_classes.get(name) {
            for class_name in classes {
                if let Some(cs) = self.class_defs.get(class_name) {
                    merge(&mut style, cs);
                }
            }
        }
        if let Some(ns) = self.node_styles.get(name) {
            merge(&mut style, ns);
        }
        (style != Style::default()).then_some(style)
    }
}

/// 先頭の空白で「id 群 / クラス名」「id 群 / プロパティ群」を 2 分割する
fn split_first_ws(s: &str) -> (&str, &str) {
    match s.find(char::is_whitespace) {
        Some(i) => (&s[..i], s[i..].trim_start()),
        None => (s, ""),
    }
}

/// 本体形状に付けるインラインスタイル属性 ` style="fill:…;stroke:…"`。
/// fill/stroke/stroke-width/stroke-dasharray のうち指定されたものだけを並べる。
/// 該当プロパティが 1 つもなければ空文字（既存スナップショットに差分を出さない）
pub(crate) fn box_attr(style: Option<&Style>) -> String {
    let Some(s) = style else {
        return String::new();
    };
    let mut decls: Vec<String> = Vec::new();
    if let Some(v) = &s.fill {
        decls.push(format!("fill:{}", escape_xml(v)));
    }
    if let Some(v) = &s.stroke {
        decls.push(format!("stroke:{}", escape_xml(v)));
    }
    if let Some(v) = &s.stroke_width {
        decls.push(format!("stroke-width:{}", escape_xml(v)));
    }
    if let Some(v) = &s.stroke_dasharray {
        decls.push(format!("stroke-dasharray:{}", escape_xml(v)));
    }
    fmt_style(&decls)
}

/// 区切り線・補助線用に stroke 系のみ。
/// fill を当てると線が塗り潰れて壊れるため除外する
pub(crate) fn line_attr(style: Option<&Style>) -> String {
    let Some(s) = style else {
        return String::new();
    };
    let mut decls: Vec<String> = Vec::new();
    if let Some(v) = &s.stroke {
        decls.push(format!("stroke:{}", escape_xml(v)));
    }
    if let Some(v) = &s.stroke_width {
        decls.push(format!("stroke-width:{}", escape_xml(v)));
    }
    if let Some(v) = &s.stroke_dasharray {
        decls.push(format!("stroke-dasharray:{}", escape_xml(v)));
    }
    fmt_style(&decls)
}

/// text 要素へ付けるラベル色属性 ` style="fill:…"`。決まらなければ空文字
pub(crate) fn text_attr(style: Option<&Style>) -> String {
    style
        .and_then(label_color)
        .map(|c| format!(r#" style="fill:{}""#, escape_xml(&c)))
        .unwrap_or_default()
}

/// ラベル文字色を決める。明示 `color:` が最優先。fill だけ指定された箱は
/// fill の明度から黒系/白系を自動で選ぶ — テーマ文字色のままだと、固定色の
/// 背景（例: 明色 fill）にダークモードの白文字が重なって読めなくなるため。
/// 16 進以外の fill（色名等）は明度を判定できないのでテーマ色に任せる
pub(crate) fn label_color(style: &Style) -> Option<String> {
    if let Some(c) = &style.color {
        return Some(c.clone());
    }
    let (r, g, b) = parse_hex_color(style.fill.as_deref()?)?;
    // YIQ 近似の輝度（0〜255）。128 以上 = 明色背景 → 黒系文字
    let yiq = (u32::from(r) * 299 + u32::from(g) * 587 + u32::from(b) * 114) / 1000;
    Some(if yiq >= 128 { "#1f2328" } else { "#f0f6fc" }.to_string())
}

/// `#rgb` / `#rrggbb`（`#rgba` / `#rrggbbaa` はアルファを無視）を (r, g, b) に読む
fn parse_hex_color(s: &str) -> Option<(u8, u8, u8)> {
    let hex = s.strip_prefix('#')?;
    if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    match hex.len() {
        3 | 4 => {
            let d = |i: usize| u8::from_str_radix(&hex[i..=i], 16).ok().map(|v| v * 17);
            Some((d(0)?, d(1)?, d(2)?))
        }
        6 | 8 => {
            let d = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
            Some((d(0)?, d(2)?, d(4)?))
        }
        _ => None,
    }
}

fn fmt_style(decls: &[String]) -> String {
    if decls.is_empty() {
        String::new()
    } else {
        format!(r#" style="{}""#, decls.join(";"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn style(fill: Option<&str>, color: Option<&str>) -> Style {
        Style {
            fill: fill.map(String::from),
            color: color.map(String::from),
            ..Style::default()
        }
    }

    #[test]
    fn ラベル色は明示_color_が最優先() {
        assert_eq!(
            label_color(&style(Some("#d5e7fe"), Some("#ff0000"))),
            Some("#ff0000".to_string())
        );
    }

    #[test]
    fn fill_だけ指定なら明度から黒系白系を自動で選ぶ() {
        // 明色 fill → 黒系文字（ダークモードの白文字が重なって読めない問題の対策）
        assert_eq!(
            label_color(&style(Some("#d5e7fe"), None)),
            Some("#1f2328".to_string())
        );
        // 暗色 fill → 白系文字（ライトモードの黒文字対策。#rgb 短縮形も可）
        assert_eq!(
            label_color(&style(Some("#333"), None)),
            Some("#f0f6fc".to_string())
        );
    }

    #[test]
    fn 明度を判定できない_fill_はテーマ色に任せる() {
        assert_eq!(label_color(&style(Some("lightblue"), None)), None);
        assert_eq!(label_color(&style(None, None)), None);
        assert_eq!(label_color(&style(Some("#12345"), None)), None, "桁数不正");
    }

    #[test]
    fn 十六進カラーのパース() {
        assert_eq!(parse_hex_color("#fff"), Some((255, 255, 255)));
        assert_eq!(parse_hex_color("#1f2328"), Some((0x1f, 0x23, 0x28)));
        assert_eq!(
            parse_hex_color("#1f2328cc"),
            Some((0x1f, 0x23, 0x28)),
            "アルファ無視"
        );
        assert_eq!(parse_hex_color("red"), None);
        assert_eq!(parse_hex_color("#ggg"), None);
    }

    #[test]
    fn コレクタは_default_クラス_style_の順に後勝ちで解決する() {
        let mut c = StyleCollector::default();
        c.class_def("default fill:#eee,stroke:#999");
        c.class_def("warn fill:#f96");
        c.add_inline("A", "warn");
        c.apply_style("A stroke:#111");
        let s = c.resolve("A").expect("A はスタイルを持つ");
        assert_eq!(s.fill.as_deref(), Some("#f96"), "warn が default を上書き");
        assert_eq!(s.stroke.as_deref(), Some("#111"), "style 文が最優先");
        // クラス未適用の箱は default だけが乗る
        let b = c.resolve("B").expect("default で B もスタイルを持つ");
        assert_eq!(b.fill.as_deref(), Some("#eee"));
        assert_eq!(b.stroke.as_deref(), Some("#999"));
    }

    #[test]
    fn 空コレクタは_none_を返す() {
        let c = StyleCollector::default();
        assert!(c.is_empty());
        assert_eq!(c.resolve("X"), None);
    }
}
