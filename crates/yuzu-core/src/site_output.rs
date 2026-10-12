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
    /// ファイルの生成物を先に照合する。`_assets/css/syntect.css` は常に出す `_assets/` の
    /// 中にあるので、ディレクトリの前方一致を先にすると、ハイライトを止めていても
    /// 有効なリンク先になってしまう
    pub fn is_emitted_target(rel: &str, s: &OutputSwitches) -> bool {
        if let Some(file) = Self::ALL.iter().find(|o| !o.is_dir() && o.path() == rel) {
            return file.emitted(s);
        }
        Self::ALL.iter().any(|o| {
            o.is_dir()
                && o.emitted(s)
                && rel
                    .strip_prefix(o.path())
                    .is_some_and(|rest| rest.starts_with('/'))
        })
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
        // 常に出す _assets/ の中でも、止めた syntect.css は通さない
        assert!(!SiteOutput::is_emitted_target(
            "_assets/css/syntect.css",
            &s
        ));
        // ディレクトリ名の前方一致だけでは通さない
        assert!(!SiteOutput::is_emitted_target("_search", &s));
        assert!(!SiteOutput::is_emitted_target("_searchx/a", &s));
        assert!(!SiteOutput::is_emitted_target("_search/a", &ALL_OFF));
    }
}
