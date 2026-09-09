//! yuzu のレンダリング: サイトモデル → 静的 HTML サイト（`dist/`）。
//!
//! - テンプレートは minijinja（プロジェクト `theme/` → 埋め込みデフォルトテーマの
//!   順で解決）
//! - コードブロックは syntect で **CSS クラス出力**（配色はビルド時生成の
//!   `syntect.css` が担い、ライト/ダーク両対応）
//! - ` ```mermaid ` は `<pre class="mermaid">` へ変換（クライアント描画）
//! - リンク・アセット参照は `baseUrl` 付きの絶対パスへ解決
//!
//! `llms.txt` / `llms-full.txt`（正規化 md の連結）もこの crate が担う（Phase 4）。
//! `yuzu fmt` の整形コアは yuzu-core の `format_document`（Phase 6）。

mod apispec;
mod assets;
mod context;
mod css;
mod error;
mod highlight;
mod llms;
mod pipeline;
mod shared;
mod speccheck;
mod templates;
mod urls;

pub use error::RenderError;
pub use highlight::SyntectCodeRenderer;

/// 設定（yuzu-config）→ パース挙動（yuzu-core の [`yuzu_core::MarkdownOptions`]）の写像。
///
/// **ここが唯一の構築点**。以前は cli 5 箇所 ＋ render 3 箇所に同一のコピーがあり、
/// フィールドを足したときに一部だけ配線を落とすと「設定したのに効かない」になっていた。
/// yuzu-render は yuzu-config と yuzu-core の両方に依存する唯一の共通の下層なので、
/// 写像の置き場はここになる
pub fn markdown_options(cfg: &yuzu_config::Config) -> yuzu_core::MarkdownOptions {
    yuzu_core::MarkdownOptions {
        gfm: cfg.markdown.gfm,
        math: cfg.markdown.math.enabled,
        mermaid: cfg.markdown.mermaid.enabled,
        crossref_site_numbering: matches!(
            cfg.markdown.crossref.numbering,
            yuzu_config::CrossrefNumbering::Site
        ),
        glossary: glossary_options(cfg),
        search_page: search_page_options(cfg),
    }
}

/// `markdown.glossary` を yuzu-core 側の中立型へ写す。
/// 呼ぶのは [`markdown_options`] だけ（部分写像を外から組み立てさせない）
fn glossary_options(cfg: &yuzu_config::Config) -> yuzu_core::GlossaryOptions {
    let g = &cfg.markdown.glossary;
    yuzu_core::GlossaryOptions {
        terms: g.terms.clone(),
        abbr: g.abbr,
        page: g.page.clone(),
        page_title: g.page_title.clone(),
    }
}

/// `search.page` を yuzu-core 側の中立型へ写す（`glossary_options` と同じ規律）。
/// `search.enabled: false` なら空を返す = 検索が無いのに結果ページだけ出ることはない
fn search_page_options(cfg: &yuzu_config::Config) -> yuzu_core::SearchPageOptions {
    if !cfg.search.enabled {
        return yuzu_core::SearchPageOptions::default();
    }
    yuzu_core::SearchPageOptions {
        page: cfg.search.page.clone(),
        page_title: cfg.search.page_title.clone(),
    }
}
pub use llms::{generate_llms_full_txt, generate_llms_txt};
pub use pipeline::{LiveReloadMode, RenderCtx, RenderParams, render_site, validate_pages};
pub use shared::RenderShared;
pub use speccheck::validate_api_specs;
pub use urls::UrlResolver;

#[cfg(test)]
mod tests {
    /// 全フィールドが設定から配線されていること。
    ///
    /// **既定値どうしの比較では配線漏れを検出できない**（yuzu-core の既定は
    /// 「用語集なし」= `page` が空、yuzu-config の既定は `"glossary"` と、
    /// 両者の既定はそもそも別物）。そこで全フィールドに既定と違う値を入れ、
    /// 写像がそれを運んでいるかを見る。
    /// **フィールドを足したらここがコンパイルエラーになる**ように分割代入で受ける
    /// （`MarkdownOptions` は `PartialEq` を導出していないので構造体比較はできない）
    #[test]
    fn 全フィールドが設定から配線される() {
        let mut cfg = yuzu_config::Config::default();
        cfg.markdown.gfm = false;
        cfg.markdown.math.enabled = false;
        cfg.markdown.mermaid.enabled = false;
        cfg.markdown.crossref.numbering = yuzu_config::CrossrefNumbering::Site;
        cfg.markdown.glossary.terms = [("SSR".to_string(), "Server-Side Rendering".to_string())]
            .into_iter()
            .collect();
        cfg.markdown.glossary.abbr = false;
        cfg.markdown.glossary.page = "yougo".to_string();
        cfg.markdown.glossary.page_title = "用語".to_string();
        cfg.search.enabled = true;
        cfg.search.page = "kensaku".to_string();
        cfg.search.page_title = "検索結果".to_string();

        let yuzu_core::MarkdownOptions {
            gfm,
            math,
            mermaid,
            crossref_site_numbering,
            glossary,
            search_page,
        } = super::markdown_options(&cfg);

        assert!(!gfm);
        assert!(!math);
        assert!(!mermaid);
        assert!(crossref_site_numbering, "numbering: Site が運ばれていない");
        assert_eq!(
            glossary.terms.get("SSR").map(String::as_str),
            Some("Server-Side Rendering")
        );
        assert!(!glossary.abbr);
        assert_eq!(glossary.page, "yougo");
        assert_eq!(glossary.page_title, "用語");
        assert_eq!(search_page.page, "kensaku");
        assert_eq!(search_page.page_title, "検索結果");
    }

    #[test]
    fn 検索が無効なら結果ページの設定は空になる() {
        let mut cfg = yuzu_config::Config::default();
        cfg.search.page = "search".to_string();
        assert_eq!(super::search_page_options(&cfg).page, "search");
        cfg.search.enabled = false;
        assert!(
            super::search_page_options(&cfg).page.is_empty(),
            "検索が無いのに結果ページだけ出てはいけない"
        );
    }
}
