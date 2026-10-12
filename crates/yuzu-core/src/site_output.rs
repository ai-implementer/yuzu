//! dist に出すサイト単位の生成物（ページ・エイリアス・同伴アセット・public/ 以外）の一覧。
//!
//! **「何を出すか」の判断はここが唯一の定義**。書き出し（yuzu-render の pipeline）・
//! 止めた生成物の掃除（`unpublished_outputs` と cli の孤児掃除）・`yuzu check` の
//! ルート絶対リンクの有効ターゲット（linkcheck）がすべてこれを通る。
//! 以前は 3 か所が別々の一覧を持ち、`output.clean = false` ＋ `--force` で
//! syntect.css や build_id が残る・check が `/404.html` を壊れたリンクにする・
//! llms を止めても `/llms-full.txt` へのリンクが通る、と食い違っていた（10-12 レビューの R1）。
//!
//! 生成物を足すときは `site_outputs!` に variant を足す。`path` / `is_dir` / `emitted` は
//! ワイルドカードの無い match なので、どれかを書き忘れるとコンパイルが止まる。
//! [`SiteOutput::ALL`] は enum と同じマクロから作るので、登録漏れが起きない。
//! ページ単位の生成物（原稿の `.md`）は [`crate::Page::emits_page_md`] が判断する

/// 「今回出すか」を決める入力。yuzu-core は設定を知らないので、呼び出し側
/// （yuzu-render の `output_switches`）が設定と実行の文脈から埋める
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutputSwitches {
    /// `llms.enabled`
    pub llms: bool,
    /// `llms.full`（`llms` が false なら出さない）
    pub llms_full: bool,
    /// sitemap.xml を出すか（`<loc>` は絶対 URL が仕様なので、base_url がフル URL のときだけ）
    pub sitemap: bool,
    /// `markdown.highlight.enabled`
    pub highlight: bool,
    /// `search.enabled`
    pub search: bool,
    /// `build --watch` のポーリング用の build_id を出すか（読むのは autorefresh.js だけ）
    pub build_id: bool,
}

/// enum と [`SiteOutput::ALL`] を 1 つの一覧から作る
macro_rules! site_outputs {
    ($( $(#[$doc:meta])* $variant:ident, )*) => {
        /// サイト単位の生成物
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum SiteOutput {
            $( $(#[$doc])* $variant, )*
        }

        impl SiteOutput {
            /// 全 variant（宣言順）
            pub const ALL: &'static [SiteOutput] = &[$(Self::$variant),*];
        }
    };
}

site_outputs! {
    /// `404.html`（静的ホスティングが自動で使う。常に出す）
    NotFound,
    /// `llms.txt`
    LlmsTxt,
    /// `llms-full.txt`
    LlmsFullTxt,
    /// `sitemap.xml`
    Sitemap,
    /// `_assets/css/syntect.css`（コードハイライトの配色）
    SyntectCss,
    /// `__yuzu/build_id`（`build --watch` のオートリフレッシュ用）
    BuildId,
    /// `_assets/`（テーマのアセット。常に出す）
    ThemeAssets,
    /// `_search/`（検索インデックス）
    SearchIndex,
}

impl SiteOutput {
    /// dist 相対のパス（`/` 区切り。ディレクトリは末尾の `/` を付けない）
    pub const fn path(self) -> &'static str {
        match self {
            Self::NotFound => "404.html",
            Self::LlmsTxt => "llms.txt",
            Self::LlmsFullTxt => "llms-full.txt",
            Self::Sitemap => "sitemap.xml",
            Self::SyntectCss => "_assets/css/syntect.css",
            Self::BuildId => "__yuzu/build_id",
            Self::ThemeAssets => "_assets",
            Self::SearchIndex => "_search",
        }
    }

    /// 中身の顔ぶれが変わるディレクトリか（掃除は中のファイルを列挙して行う）
    pub const fn is_dir(self) -> bool {
        match self {
            Self::ThemeAssets | Self::SearchIndex => true,
            Self::NotFound
            | Self::LlmsTxt
            | Self::LlmsFullTxt
            | Self::Sitemap
            | Self::SyntectCss
            | Self::BuildId => false,
        }
    }

    /// 今回の設定と実行で出すか
    pub const fn emitted(self, s: &OutputSwitches) -> bool {
        match self {
            Self::NotFound | Self::ThemeAssets => true,
            Self::LlmsTxt => s.llms,
            Self::LlmsFullTxt => s.llms && s.llms_full,
            Self::Sitemap => s.sitemap,
            Self::SyntectCss => s.highlight,
            Self::BuildId => s.build_id,
            Self::SearchIndex => s.search,
        }
    }

    /// dist 相対パス `rel` が、今回出す生成物（またはその中のファイル）か。
    ///
    /// 今回出すもののどれかが `rel` を含めば有効。止めたファイルの生成物でも、常に出す
    /// ディレクトリの中にあれば有効にする。`_assets/` には `theme/static/` も書き出すので、
    /// ハイライトを止めても `theme/static/css/syntect.css` を置けば同じパスに出る
    /// （ファイル単位の判定を先にすると、出しているのに壊れたリンクになった。PR #52 の
    /// レビュー指摘）。`_assets/` の中身は列挙せず前方一致で通す（従来どおり）
    pub fn is_emitted_target(rel: &str, s: &OutputSwitches) -> bool {
        Self::ALL.iter().any(|o| o.emitted(s) && o.contains(rel))
    }

    /// `rel` がこの生成物（ディレクトリならその中のファイル）か
    fn contains(self, rel: &str) -> bool {
        if self.is_dir() {
            rel.strip_prefix(self.path())
                .is_some_and(|rest| rest.starts_with('/'))
        } else {
            rel == self.path()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL_ON: OutputSwitches = OutputSwitches {
        llms: true,
        llms_full: true,
        sitemap: true,
        highlight: true,
        search: true,
        build_id: true,
    };
    const ALL_OFF: OutputSwitches = OutputSwitches {
        llms: false,
        llms_full: false,
        sitemap: false,
        highlight: false,
        search: false,
        build_id: false,
    };

    #[test]
    fn パスは重複しない() {
        let mut seen = std::collections::HashSet::new();
        for output in SiteOutput::ALL {
            assert!(seen.insert(output.path()), "{output:?}");
            assert!(!output.path().ends_with('/'), "{output:?}");
        }
    }

    #[test]
    fn 常に出すものと切り替えで止まるもの() {
        let always: Vec<_> = SiteOutput::ALL
            .iter()
            .filter(|o| o.emitted(&ALL_OFF))
            .collect();
        assert_eq!(always, [&SiteOutput::NotFound, &SiteOutput::ThemeAssets]);
        assert!(SiteOutput::ALL.iter().all(|o| o.emitted(&ALL_ON)));
        // llms-full.txt は llms 自体を止めたら出さない
        let full_only = OutputSwitches {
            llms: false,
            ..ALL_ON
        };
        assert!(!SiteOutput::LlmsFullTxt.emitted(&full_only));
    }

    #[test]
    fn リンク先の照合は出すものだけを通す() {
        let s = OutputSwitches {
            llms: false,
            highlight: false,
            ..ALL_ON
        };
        assert!(SiteOutput::is_emitted_target("404.html", &s));
        assert!(SiteOutput::is_emitted_target("sitemap.xml", &s));
        assert!(SiteOutput::is_emitted_target("_search/manifest.json", &s));
        assert!(SiteOutput::is_emitted_target("_assets/css/theme.css", &s));
        assert!(!SiteOutput::is_emitted_target("llms.txt", &s));
        assert!(!SiteOutput::is_emitted_target("llms-full.txt", &s));
        // 止めた syntect.css でも、常に出す _assets/ の中なので通す（theme/static/ が
        // 同じパスに書き出せる。PR #52 のレビュー指摘）
        assert!(SiteOutput::is_emitted_target("_assets/css/syntect.css", &s));
        // ディレクトリ名の前方一致だけでは通さない
        assert!(!SiteOutput::is_emitted_target("_search", &s));
        assert!(!SiteOutput::is_emitted_target("_searchx/a", &s));
        assert!(!SiteOutput::is_emitted_target("_search/a", &ALL_OFF));
    }
}
