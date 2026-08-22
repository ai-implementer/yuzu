//! `yuzu.toml` ⇔ [`Config`] の手書き変換（kabosu の `Decode` / `Encode` 実装）。
//!
//! - キーは snake_case（構造体のフィールド名と同じ）。`table_codec!` がキー名と
//!   フィールドを 1 行で対応付けるので、decode と encode の集合がズレない
//! - すべてのキーは省略可能。欠落は各 `Default` の値で埋める
//! - 未知キーの方針（Deny）は `resolve::load` が `DecodeOptions` で指定し、
//!   ここでは `TableDecoder::finish` に委ねる
//! - 列挙値（`page` / `site` 等）と `lint.rules` のルール ID は独自診断で検証する。
//!   独自診断の文言は日本語で書く（kabosu の組み込み文言は英語で、`resolve` が翻訳する）
//! - `Encode` は envKey 用の正規化出力（[`Config::to_toml`]）に使う。同じ値からは
//!   常に同じバイト列が出る（kabosu の正規化出力の保証）

use std::collections::BTreeMap;

use kabosu::{
    Decode, DecodeContext, Diagnostic, DiagnosticCode, Encode, EncodeError, Encoder, KeySegment,
    Node, Severity, TableDecoder,
};

use crate::schema::{
    BuildConfig, Config, CrossrefConfig, CrossrefNumbering, DISABLEABLE_RULES, DevConfig,
    GitConfig, GlossaryConfig, HighlightConfig, InputConfig, LintConfig, LlmsConfig,
    MarkdownConfig, MathConfig, MermaidBackend, MermaidConfig, NavConfig, OutputConfig,
    SearchConfig, ShardConfig, SiteConfig, ThemeConfig, TocConfig, TypoToleranceConfig,
};

/// テーブル型の `Decode` / `Encode` を「キー名 => フィールド」の対応 1 行ずつで定義する。
/// すべて任意キーで、欠落は `Default` の値。`Option` フィールドは欠落 = None、
/// encode では None ならキーを省略する（kabosu の `Option` 実装）
macro_rules! table_codec {
    ($ty:ident { $( $key:literal => $field:ident ),* $(,)? }) => {
        impl Decode for $ty {
            fn decode(node: &Node, cx: &mut DecodeContext<'_>) -> Option<Self> {
                let mut d = TableDecoder::new(node, cx)?;
                let defaults = $ty::default();
                $( let $field = d.optional($key); )*
                d.finish();
                Some(Self {
                    $( $field: $field.unwrap_or(defaults.$field), )*
                })
            }
        }

        impl Encode for $ty {
            fn encode(&self, encoder: &mut Encoder<'_>) -> Result<(), EncodeError> {
                let mut t = encoder.table();
                $( t.field($key, &self.$field)?; )*
                Ok(())
            }
        }
    };
}

table_codec!(Config {
    "site" => site,
    "input" => input,
    "output" => output,
    "theme" => theme,
    "nav" => nav,
    "markdown" => markdown,
    "lint" => lint,
    "search" => search,
    "llms" => llms,
    "build" => build,
    "dev" => dev,
    "git" => git,
});

table_codec!(SiteConfig {
    "title" => title,
    "description" => description,
    "base_url" => base_url,
    "lang" => lang,
    "logo" => logo,
});

table_codec!(InputConfig {
    "dir" => dir,
    "ignore" => ignore,
});

table_codec!(OutputConfig {
    "dir" => dir,
    "clean" => clean,
});

table_codec!(ThemeConfig {
    "name" => name,
    "dark" => dark,
    "css_vars" => css_vars,
    "css_vars_dark" => css_vars_dark,
    "toc" => toc,
});

table_codec!(TocConfig {
    "levels" => levels,
});

table_codec!(NavConfig {
    "auto" => auto,
    "collapse" => collapse,
});

table_codec!(MarkdownConfig {
    "gfm" => gfm,
    "highlight" => highlight,
    "mermaid" => mermaid,
    "math" => math,
    "crossref" => crossref,
    "glossary" => glossary,
});

table_codec!(HighlightConfig {
    "enabled" => enabled,
    "theme_light" => theme_light,
    "theme_dark" => theme_dark,
    "line_numbers" => line_numbers,
});

table_codec!(MermaidConfig {
    "enabled" => enabled,
    "backend" => backend,
});

table_codec!(MathConfig {
    "enabled" => enabled,
});

table_codec!(CrossrefConfig {
    "numbering" => numbering,
});

table_codec!(GlossaryConfig {
    "terms" => terms,
    "abbr" => abbr,
    "page" => page,
    "page_title" => page_title,
});

table_codec!(SearchConfig {
    "enabled" => enabled,
    "dictionary" => dictionary,
    "typo_tolerance" => typo_tolerance,
    "shard" => shard,
    "synonyms" => synonyms,
    "index_code" => index_code,
    "page" => page,
    "page_title" => page_title,
    "page_size" => page_size,
});

table_codec!(TypoToleranceConfig {
    "enabled" => enabled,
    "max_edits" => max_edits,
});

table_codec!(ShardConfig {
    "max_terms_per_shard" => max_terms_per_shard,
});

table_codec!(LlmsConfig {
    "enabled" => enabled,
    "full" => full,
});

table_codec!(BuildConfig {
    "base_url" => base_url,
    "watch_ignore" => watch_ignore,
});

table_codec!(DevConfig {
    "host" => host,
    "port" => port,
    "live_reload" => live_reload,
    "open" => open,
});

table_codec!(GitConfig {
    "last_updated" => last_updated,
    "edit_url" => edit_url,
});

// ---- 列挙値（文字列の選択肢） ----

/// 文字列の選択肢を decode する。選択肢に無い値は位置付きの Error 診断
/// （指定できる値の一覧入り）にして None
fn decode_choice<T: Copy>(
    node: &Node,
    cx: &mut DecodeContext<'_>,
    choices: &[(&str, T)],
) -> Option<T> {
    let found = String::decode(node, cx)?;
    if let Some((_, value)) = choices.iter().find(|(name, _)| *name == found) {
        return Some(*value);
    }
    let names = choices
        .iter()
        .map(|(name, _)| format!("`{name}`"))
        .collect::<Vec<_>>()
        .join(" / ");
    let path = cx.key_path().clone();
    cx.diagnostic(Diagnostic::new(
        DiagnosticCode::Custom("invalid-value".into()),
        Severity::Error,
        format!("`{path}` の値 `{found}` は使えません（指定できる値: {names}）"),
        path,
        node.span(),
    ));
    None
}

impl Decode for CrossrefNumbering {
    fn decode(node: &Node, cx: &mut DecodeContext<'_>) -> Option<Self> {
        decode_choice(node, cx, &[("page", Self::Page), ("site", Self::Site)])
    }
}

impl Encode for CrossrefNumbering {
    fn encode(&self, encoder: &mut Encoder<'_>) -> Result<(), EncodeError> {
        encoder.string(self.as_str());
        Ok(())
    }
}

impl Decode for MermaidBackend {
    fn decode(node: &Node, cx: &mut DecodeContext<'_>) -> Option<Self> {
        decode_choice(node, cx, &[("client", Self::Client), ("ssr", Self::Ssr)])
    }
}

impl Encode for MermaidBackend {
    fn encode(&self, encoder: &mut Encoder<'_>) -> Result<(), EncodeError> {
        encoder.string(self.as_str());
        Ok(())
    }
}

// ---- lint（rules のルール ID を検証するため手書き） ----

/// `lint.rules`。自由キーのマップとして読んだ後、キーが無効化できるルール ID か
/// 検証する（タイポ・旧形式のキー・無効化不可の ID を位置付きで弾く。
/// 黙って受理すると「無効化したつもりが効いていない」事故になる）
struct LintRules(BTreeMap<String, bool>);

impl Decode for LintRules {
    fn decode(node: &Node, cx: &mut DecodeContext<'_>) -> Option<Self> {
        let map = BTreeMap::<String, bool>::decode(node, cx)?;
        // BTreeMap の decode が通ったので node はテーブル
        let table = node.as_table()?;
        let mut ok = true;
        for id in map.keys() {
            if DISABLEABLE_RULES.contains(&id.as_str()) {
                continue;
            }
            let span = table.get(id).map_or(node.span(), |e| e.key_span());
            let mut path = cx.key_path().clone();
            path.push(KeySegment::new(id.clone(), span));
            cx.diagnostic(Diagnostic::new(
                DiagnosticCode::Custom("unknown-rule".into()),
                Severity::Error,
                format!(
                    "`{path}` は無効化できるルール ID ではありません（指定できる ID: {}）",
                    DISABLEABLE_RULES.join(", ")
                ),
                path,
                span,
            ));
            ok = false;
        }
        ok.then_some(Self(map))
    }
}

impl Decode for LintConfig {
    fn decode(node: &Node, cx: &mut DecodeContext<'_>) -> Option<Self> {
        let mut d = TableDecoder::new(node, cx)?;
        let defaults = Self::default();
        let max_directory_depth = d.optional("max_directory_depth");
        let terms = d.optional("terms");
        let rules = d.optional::<LintRules>("rules");
        d.finish();
        Some(Self {
            max_directory_depth: max_directory_depth.unwrap_or(defaults.max_directory_depth),
            terms: terms.unwrap_or(defaults.terms),
            rules: rules.map_or(defaults.rules, |r| r.0),
        })
    }
}

impl Encode for LintConfig {
    fn encode(&self, encoder: &mut Encoder<'_>) -> Result<(), EncodeError> {
        let mut t = encoder.table();
        t.field("max_directory_depth", &self.max_directory_depth)?;
        t.field("terms", &self.terms)?;
        t.field("rules", &self.rules)?;
        Ok(())
    }
}

impl Config {
    /// 正規形の TOML 文字列（同じ値からは常に同じバイト列。envKey などのハッシュ用）。
    /// コメントは含まない
    pub fn to_toml(&self) -> String {
        kabosu::to_string(self).expect("設定は常に TOML 化できる")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 既定値は_toml_化して読み戻せる() {
        let toml = Config::default().to_toml();
        let doc = kabosu::Document::parse(&toml).expect("正規化出力は再パースできる");
        let mut options = kabosu::DecodeOptions::default();
        options.unknown_keys = kabosu::UnknownKeys::Deny;
        let report = kabosu::decode::<Config>(&doc, options);
        assert!(!report.has_errors(), "{:?}", report.diagnostics());
        // 既定値の往復で同じバイト列になる（決定的）
        assert_eq!(report.value().unwrap().to_toml(), toml);
    }

    #[test]
    fn 正規化出力は_snake_case_のキーで_none_を省略する() {
        let toml = Config::default().to_toml();
        assert!(toml.contains("[dev]\n"), "{toml}");
        assert!(toml.contains("live_reload = true"), "{toml}");
        assert!(toml.contains("[markdown.mermaid]\n"), "{toml}");
        assert!(toml.contains("backend = \"client\""), "{toml}");
        assert!(!toml.contains("edit_url"), "None のキーは出ない: {toml}");
        assert!(!toml.contains("baseUrl"), "camelCase は残らない: {toml}");
    }
}
