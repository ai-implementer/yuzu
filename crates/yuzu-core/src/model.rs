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
    /// Markdown 原文（本文 HTML 化・将来の `yuzu fmt` が再パースに使う）
    pub source: String,
}

impl Page {
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
