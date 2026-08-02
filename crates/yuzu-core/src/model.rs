//! パーサ非依存の公開ドキュメントモデル

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// frontmatter（YAML、`---` 区切り）で指定できるページメタデータ。
/// 未知のキーは無視する（後続フェーズで `slug` / `tags` 等を追加予定）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Frontmatter {
    /// ページタイトル（ナビ表示にも使う）。未指定なら先頭 h1 → ファイル名の順で補う
    pub title: Option<String>,
    /// ナビの並び順（昇順）。未指定はファイル名順で最後尾グループ
    pub order: Option<i64>,
    /// true ならビルド対象から除外
    pub draft: bool,
    /// メタディスクリプション
    pub description: Option<String>,
    /// false なら llms.txt / llms-full.txt に収録しない
    pub llms: bool,
    /// このページへリダイレクトする旧 URL（route 形式。例 `guide/old-name/`。
    /// 先頭 `/`・末尾スラッシュ省略は正規化で吸収）。ビルド時に各エイリアスへ
    /// リダイレクト HTML を生成する。実ページや他エイリアスとの衝突はエラー
    pub aliases: Vec<String>,
}

// llms の既定を true にするため derive ではなく手書き
// （serde のコンテナ #[serde(default)] もこの Default を使う）
impl Default for Frontmatter {
    fn default() -> Self {
        Self {
            title: None,
            order: None,
            draft: false,
            description: None,
            llms: true,
            aliases: Vec::new(),
        }
    }
}

/// ソース上の位置（1 始まりの行・列）。将来の Linter 診断用に保持する
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceSpan {
    pub start_line: usize,
    pub start_col: usize,
    pub end_line: usize,
    pub end_col: usize,
}

/// 図表キャプションのラベル（相互参照のターゲット）。
/// `Figure: 説明 {#fig:deps}` から採番と id を確定して収集する
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossrefLabel {
    /// `{#fig:deps}` の中身（アンカー id と同じ値）
    pub id: String,
    /// 種別（図 / 表 / リスト）。参照テキストの自動補完に使う
    pub kind: crate::markdown::crossref::CaptionKind,
    /// ページ内の採番（種別ごとに 1 から）
    pub number: usize,
    /// キャプション本文
    pub text: String,
    /// ソース上の位置（重複ラベルの診断用）
    pub span: SourceSpan,
}

/// ページ内 TOC の 1 エントリ（見出し）。
/// ID は本文 HTML の見出しアンカーと一致することを保証する
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TocEntry {
    /// 見出しレベル（1〜6）。表示対象の絞り込みは利用側で行う
    pub level: u8,
    /// アンカー ID（`<h2 id="...">` と同じ値）
    pub id: String,
    /// 見出しのプレーンテキスト
    pub text: String,
    /// ソース上の位置
    pub span: SourceSpan,
}

/// 検索インデックス用のセクション（h2/h3 境界で分割したプレーンテキスト）
#[derive(Debug, Clone, PartialEq)]
pub struct PlainSection {
    /// 見出しのアンカー ID（本文 HTML の `<h2 id="...">` と同一）。リード文は None
    pub anchor: Option<String>,
    /// 見出しのプレーンテキスト。リード文は None
    pub heading: Option<String>,
    /// セクション本文。h2/h3 自身の見出しテキストは含まない
    /// （インデクサが heading フィールドに重みを付けて別計上する）。
    /// h1・h4〜h6 の見出しテキストは本文として含む（検索対象に残す）
    pub body: String,
}

/// 1 つの Markdown ページ
#[derive(Debug, Clone)]
pub struct Page {
    /// ソースファイルの絶対パス
    pub src: PathBuf,
    /// `content/` からの相対パス（例: `guide/getting-started.md`）
    pub rel: PathBuf,
    /// サイト相対 URL。base path は含まず、`""`（トップ）または
    /// `"guide/getting-started/"` のように末尾スラッシュ付き
    pub route: String,
    pub frontmatter: Frontmatter,
    /// 解決済みタイトル（frontmatter → 先頭 h1 → ファイル名の優先順）
    pub title: String,
    /// ページ内 TOC（h1〜h6 全見出し）
    pub toc: Vec<TocEntry>,
    /// 図表キャプションのラベル（相互参照のターゲット。文書順）
    pub labels: Vec<CrossrefLabel>,
    /// 図表番号の開始オフセット（種別ごとの先行ページまでの個数）。
    /// `markdown.crossref.numbering: "site"` のときだけ非ゼロになる
    pub crossref_offset: crate::markdown::crossref::Numbering,
    /// Markdown 原文（本文 HTML 化・将来の `yuzu fmt` が再パースに使う）
    pub source: String,
    /// ビルド時に合成したページの種別（実ページは None）。**実ファイルが無い**ので
    /// `yuzu fmt` / `yuzu lint --fix` の書き込み対象から外し、「このページを編集」
    /// リンクも出さない。リンク検査では**リンク先としてだけ**有効にする。
    /// 集約（nav・検索索引・sitemap・ページ単位 .md）に載せるかは種別ごとに違うため、
    /// 呼び出し側は kind を直接見ず `Page::in_*` ヘルパを通す
    pub generated: Option<GeneratedKind>,
}

/// ビルド時に合成されるページの種別
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeneratedKind {
    /// 用語集ページ（`markdown.glossary.page`）
    Glossary,
    /// 検索結果ページ（`search.page`）
    Search,
}

impl GeneratedKind {
    /// このページの route を決める設定キー（診断文面用の唯一の定義）
    pub fn config_key(self) -> &'static str {
        match self {
            Self::Glossary => "markdown.glossary.page",
            Self::Search => "search.page",
        }
    }

    /// 診断文面での呼び名（「自動生成される◯◯」に続く名詞）
    pub fn label(self) -> &'static str {
        match self {
            Self::Glossary => "用語集ページ",
            Self::Search => "検索結果ページ",
        }
    }

    /// コンテンツ集約（nav・検索索引・sitemap・ページ単位 .md）に載せるか。
    /// 用語集は読み物なので載せる。検索結果ページは中身が実行時に決まる
    /// 機能ページなので載せない（JS 前提の空ページを集約に混ぜない）
    fn in_listings(self) -> bool {
        match self {
            Self::Glossary => true,
            Self::Search => false,
        }
    }
}

impl Page {
    /// 合成ページか（`page.src` が実在しない）。`yuzu fmt` / `lint --fix` の
    /// 書き込み防止・lint / 診断の除外・`edit_url` 抑止・集計行の分母はこれを見る
    pub fn is_generated(&self) -> bool {
        self.generated.is_some()
    }

    /// サイドバー nav（と、その派生の pager・パンくず）に載せるか
    pub fn in_nav(&self) -> bool {
        self.generated.is_none_or(GeneratedKind::in_listings)
    }

    /// 検索インデックスに載せるか
    pub fn in_search_index(&self) -> bool {
        self.generated.is_none_or(GeneratedKind::in_listings)
    }

    /// sitemap.xml に載せるか
    pub fn in_sitemap(&self) -> bool {
        self.generated.is_none_or(GeneratedKind::in_listings)
    }

    /// ページ単位 Markdown（`md_rel_path()`）を出力するか
    pub fn emits_page_md(&self) -> bool {
        self.generated.is_none_or(GeneratedKind::in_listings)
    }

    /// 出力ファイルの相対パス（pretty URL: `route + "index.html"`）
    pub fn output_rel_path(&self) -> String {
        format!("{}index.html", self.route)
    }

    /// ページ単位 Markdown の配信相対パス。
    /// route の末尾スラッシュを落として `.md` を付ける（`guide/intro/` → `guide/intro.md`）。
    /// ルート（route 空）は `index.md`。HTML と競合しない（`<route>index.html` はディレクトリ内）
    pub fn md_rel_path(&self) -> String {
        if self.route.is_empty() {
            "index.md".to_string()
        } else {
            format!("{}.md", self.route.trim_end_matches('/'))
        }
    }
}

/// ナビツリーの 1 ノード（ページ、またはページを束ねるディレクトリ）
#[derive(Debug, Clone, Serialize)]
pub struct NavNode {
    pub title: String,
    /// リンク先 route。`index.md` を持たないディレクトリは None（ラベルのみ）
    pub route: Option<String>,
    /// frontmatter の並び順（ディレクトリは配下 `index.md` の値）
    pub order: Option<i64>,
    pub children: Vec<NavNode>,
}

/// サイト全体のモデル
#[derive(Debug, Clone)]
pub struct SiteModel {
    /// 全ページ（draft 除外済み、走査順＝パスのソート順）
    pub pages: Vec<Page>,
    /// ナビツリー（`order` → 名前順でソート済み）
    pub nav: Vec<NavNode>,
}

impl SiteModel {
    /// `content/` からの相対パス（`/` 区切り）→ route の解決。
    /// 本文中の `.md` 相互リンクの解決に使う
    pub fn route_for_rel_str(&self, rel: &str) -> Option<&str> {
        self.pages
            .iter()
            .find(|p| crate::urlpath::rel_to_slash(&p.rel) == rel)
            .map(|p| p.route.as_str())
    }
}
