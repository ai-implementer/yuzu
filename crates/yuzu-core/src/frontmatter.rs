//! frontmatter（YAML）のパース

use crate::model::Frontmatter;

/// [`Frontmatter`] が受理するトップレベルキー（lint の未知キー検出用）。
/// フィールドを増やすときはここにも足す（乖離は下のテストで検知する）
pub(crate) const KNOWN_KEYS: &[&str] = &["title", "order", "draft", "description", "llms"];

/// comrak の front matter extension が切り出した生テキスト
/// （`---` 区切り行を含む）から YAML 部分を取り出してパースする
pub(crate) fn parse_frontmatter(raw: &str) -> Result<Frontmatter, String> {
    let body = yaml_body(raw);
    if body.is_empty() {
        return Ok(Frontmatter::default());
    }
    serde_yaml_ng::from_str(body).map_err(|e| e.to_string())
}

/// 生テキストから `---` 区切りを外した YAML 部分を返す
pub(crate) fn yaml_body(raw: &str) -> &str {
    let trimmed = raw.trim();
    trimmed
        .strip_prefix("---")
        .and_then(|s| s.strip_suffix("---"))
        .unwrap_or(trimmed)
        .trim()
}

#[cfg(test)]
mod tests {
    use super::parse_frontmatter;

    #[test]
    fn 基本キーをパースできる() {
        let fm = parse_frontmatter("---\ntitle: はじめに\norder: 2\ndraft: true\n---\n").unwrap();
        assert_eq!(fm.title.as_deref(), Some("はじめに"));
        assert_eq!(fm.order, Some(2));
        assert!(fm.draft);
        assert!(fm.description.is_none());
    }

    #[test]
    fn 空の_frontmatter_はデフォルトになる() {
        let fm = parse_frontmatter("---\n---\n").unwrap();
        assert!(fm.title.is_none());
        assert!(!fm.draft);
    }

    #[test]
    fn llms_は省略時_true_で_false_を指定できる() {
        let fm = parse_frontmatter("---\ntitle: x\n---\n").unwrap();
        assert!(fm.llms, "省略時は収録する");
        let fm = parse_frontmatter("---\nllms: false\n---\n").unwrap();
        assert!(!fm.llms);
    }

    #[test]
    fn 未知のキーは無視する() {
        let fm = parse_frontmatter("---\ntitle: x\nfuture_key: 123\n---\n").unwrap();
        assert_eq!(fm.title.as_deref(), Some("x"));
    }

    #[test]
    fn 不正な_yaml_はエラーになる() {
        assert!(parse_frontmatter("---\ntitle: [unclosed\n---\n").is_err());
    }

    /// KNOWN_KEYS と Frontmatter 構造体の乖離検知
    /// （フィールドを追加して KNOWN_KEYS を忘れると未知キー lint が誤検知する）
    #[test]
    fn known_keys_は_frontmatter_のフィールドと一致する() {
        let yaml = serde_yaml_ng::to_string(&crate::model::Frontmatter::default()).unwrap();
        let value: serde_yaml_ng::Value = serde_yaml_ng::from_str(&yaml).unwrap();
        let mut fields: Vec<String> = value
            .as_mapping()
            .unwrap()
            .keys()
            .map(|k| k.as_str().unwrap().to_string())
            .collect();
        fields.sort();
        let mut known: Vec<String> = super::KNOWN_KEYS.iter().map(|k| k.to_string()).collect();
        known.sort();
        assert_eq!(fields, known);
    }
}
