//! パーサ非依存の公開ドキュメントモデル

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// frontmatter（YAML、`---` 区切り）で指定できるページメタデータ。
/// 未知のキーは無視する（後続フェーズで `slug` / `tags` / `llms` 等を追加予定）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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
}

/// ソース上の位置（1 始まりの行・列）。将来の Linter 診断用に保持する
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct SourceSpan {
    pub start_line: usize,
    pub start_col: usize,
    pub end_line: usize,
    pub end_col: usize,
}

/// ページ内 TOC の 1 エントリ（見出し）。
/// ID は本文 HTML の見出しアンカーと一致することを保証する
#[derive(Debug, Clone, Serialize)]
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
            .find(|p| crate::scan::rel_to_slash(&p.rel) == rel)
            .map(|p| p.route.as_str())
    }
}
