//! frontmatter（YAML）のパース

use crate::model::Frontmatter;

/// comrak の front matter extension が切り出した生テキスト
/// （`---` 区切り行を含む）から YAML 部分を取り出してパースする
pub(crate) fn parse_frontmatter(raw: &str) -> Result<Frontmatter, String> {
    let trimmed = raw.trim();
    let body = trimmed
        .strip_prefix("---")
        .and_then(|s| s.strip_suffix("---"))
        .unwrap_or(trimmed)
        .trim();

    if body.is_empty() {
        return Ok(Frontmatter::default());
    }
    serde_yaml_ng::from_str(body).map_err(|e| e.to_string())
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
}
