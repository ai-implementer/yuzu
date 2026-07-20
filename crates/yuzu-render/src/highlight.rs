//! syntect によるビルド時シンタックスハイライト（CSS クラス出力）と
//! ` ```mermaid ` ブロックの変換。
//!
//! 凍結事項: インラインスタイルは使わない。配色はテーマ CSS
//! （ビルド時生成の `syntect.css`、ライト/ダーク両対応）が担う。
//!
//! Mermaid は backend 設定により:
//! - client: `<pre class="mermaid">`（mermaid.js クライアント描画。従来どおり）
//! - ssr: tankan でビルド時 SVG 化。未対応図種・構文はクライアント描画へ
//!   フォールバックし、その事実をページ単位で記録する（mermaid.js の読込判定用）

use std::cell::Cell;
use std::path::{Path, PathBuf};

use syntect::html::{ClassStyle, ClassedHTMLGenerator};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

use yuzu_config::{HighlightConfig, MermaidBackend, MermaidConfig};
use yuzu_core::{CodeBlockMeta, CodeBlockRenderer};

use crate::apispec::{self, SpecFiles, SpecKind};

/// syntect のクラス名接頭辞。テーマ CSS のクラスと衝突しないようにする。
/// `css.rs` の CSS 生成と必ず同じ値を使うこと
pub(crate) const CLASS_STYLE: ClassStyle = ClassStyle::SpacedPrefixed { prefix: "yz-" };

/// 共有側（ページ横断で不変）のハイライタ。
/// - `lang == "mermaid"` → SSR（tankan）またはクライアント描画へ
/// - 既知の言語 → syntect ハイライト（CSS クラス）
/// - 言語なし・未知の言語 → `None`（パーサ既定のエスケープ済み `<pre><code>`）
///
/// ページ内状態（SVG 連番・フォールバック・外部依存）は持たない。
/// 実際のレンダリングは [`SyntectCodeRenderer::page_renderer`] で作る
/// ページローカルな [`PageCodeRenderer`] が行う
pub struct SyntectCodeRenderer {
    syntax_set: SyntaxSet,
    highlight_enabled: bool,
    /// 行番号表示のサイト既定（`markdown.highlight.lineNumbers`。
    /// ブロック単位の `showLineNumbers` / `noLineNumbers` が優先される）
    line_numbers_default: bool,
    mermaid_enabled: bool,
    /// backend == Ssr のときだけ Some（tankan オプションの雛形）
    mermaid_ssr_options: Option<tankan::Options>,
    /// `file:` 参照の基準ディレクトリ（プロジェクトルート）。
    /// 未設定（単体テスト等）ではファイル参照をエラーボックスにする
    apispec_root: Option<PathBuf>,
}

/// ページ単位の [`CodeBlockRenderer`]。共有の [`SyntectCodeRenderer`] を参照しつつ、
/// ページ内状態（SVG 連番・フォールバック・外部依存フラグ）を自分で持つ。
/// `Cell` は `!Sync` なので、ページ並列化でこの状態をスレッド間共有してしまう
/// 事故は型で防がれる（各ページが自分の PageCodeRenderer を作って使う）
pub struct PageCodeRenderer<'a> {
    shared: &'a SyntectCodeRenderer,
    /// このページでクライアント描画へのフォールバックが発生したか
    mermaid_fallback: Cell<bool>,
    /// ページ内の SVG 連番（`<marker>` id の一意化用）
    mermaid_counter: Cell<usize>,
    /// このページで外部ファイル参照を使ったか（= 本文キャッシュ不可の印）
    external_deps: Cell<bool>,
}

/// apispec への仕様ファイル読み込み口。canonicalize によるルート配下の強制と、
/// 「このページは外部ファイルに依存した」の記録（本文キャッシュ非対象化）を担う
struct ProjectSpecFiles<'a> {
    root: Option<&'a Path>,
    external_deps: &'a Cell<bool>,
}

impl apispec::SpecFiles for ProjectSpecFiles<'_> {
    fn read(&self, rel: &str) -> Result<String, String> {
        // 読み込みの単一チョークポイント: `file:` 参照も文書内のファイル $ref も
        // 必ずここを通るため、依存フラグの立て漏れが構造的に起きない
        self.external_deps.set(true);
        let Some(root) = self.root else {
            return Err(
                "このビルドではファイル参照が使えません（基準ディレクトリ未設定）".to_string(),
            );
        };
        let root = root
            .canonicalize()
            .map_err(|e| format!("プロジェクトルートを解決できません: {e}"))?;
        let path = root.join(rel);
        let canonical = path
            .canonicalize()
            .map_err(|e| format!("仕様ファイル {rel} を読めません: {e}"))?;
        if !canonical.starts_with(&root) {
            return Err(format!(
                "仕様ファイル {rel} はプロジェクトルートの外を指しています"
            ));
        }
        std::fs::read_to_string(&canonical)
            .map_err(|e| format!("仕様ファイル {rel} を読めません: {e}"))
    }
}

impl SyntectCodeRenderer {
    pub fn new(highlight: &HighlightConfig, mermaid: &MermaidConfig) -> Self {
        let mermaid_ssr_options =
            (mermaid.enabled && mermaid.backend == MermaidBackend::Ssr).then(|| tankan::Options {
                // テーマ CSS の変数に追従させる（インライン SVG 前提。
                // ダーク切替 = html[data-theme] の変数上書きに再描画なしで追従）
                theme: tankan::Theme {
                    foreground: "var(--fg, #1f2328)".to_string(),
                    muted: "var(--fg-muted, #59636e)".to_string(),
                    background: "var(--bg, #ffffff)".to_string(),
                    surface: "var(--bg-subtle, #f6f8fa)".to_string(),
                    border: "var(--border, #d1d9e0)".to_string(),
                    accent: "var(--accent-fg, #9a6700)".to_string(),
                },
                // theme.css の body と同じフォントスタック
                font_family: "-apple-system, BlinkMacSystemFont, 'Segoe UI', \
                              'Hiragino Sans', 'Noto Sans JP', Meiryo, sans-serif"
                    .to_string(),
                ..tankan::Options::default()
            });
        Self {
            // ClassedHTMLGenerator の行単位 API は newlines 版とセットで使う。
            // syntect デフォルトには TypeScript/TSX/TOML/Dockerfile 等がないため、
            // bat のアセット由来の拡張セット（two-face。デフォルト構文も内包）を使う
            syntax_set: two_face::syntax::extra_newlines(),
            highlight_enabled: highlight.enabled,
            line_numbers_default: highlight.line_numbers,
            mermaid_enabled: mermaid.enabled,
            mermaid_ssr_options,
            apispec_root: None,
        }
    }

    /// `file:` 参照の基準ディレクトリ（プロジェクトルート）を設定する
    pub fn set_project_root(&mut self, root: PathBuf) {
        self.apispec_root = Some(root);
    }

    /// ページ 1 枚ぶんのレンダラを作る（ページ内状態は初期値で始まる）
    pub fn page_renderer(&self) -> PageCodeRenderer<'_> {
        PageCodeRenderer {
            shared: self,
            mermaid_fallback: Cell::new(false),
            mermaid_counter: Cell::new(0),
            external_deps: Cell::new(false),
        }
    }

    /// syntect ハイライト（CSS クラス出力の一括 HTML）。
    /// 未知の言語・失敗は `None`（呼び出し側がプレーン表示へフォールバック）
    fn highlight(&self, lang: &str, code: &str) -> Option<String> {
        let syntax = self.syntax_set.find_syntax_by_token(lang)?;
        let mut generator =
            ClassedHTMLGenerator::new_with_class_style(syntax, &self.syntax_set, CLASS_STYLE);
        for line in LinesWithEndings::from(code) {
            if let Err(e) = generator.parse_html_for_line_which_includes_newline(line) {
                // ハイライト失敗でビルドは止めず、プレーン表示へフォールバック
                tracing::warn!(lang, error = %e, "ハイライトに失敗したためプレーン表示にする");
                return None;
            }
        }
        Some(generator.finalize())
    }
}

/// ハイライト済み HTML を行ごとの自己完結な断片へ分割する（改行区切りは除去）。
/// 行をまたいで開いている `<span>` は行末で閉じ、次の行頭で同じタグを開き直す
/// （入力はこのモジュールが生成した span とエスケープ済みテキストだけ、が前提）。
/// 末尾の改行の後には行を作らない（`"a\n"` → 1 行、`"a\n\n"` → 2 行）
fn split_lines_balanced(html: &str) -> Vec<String> {
    let mut result = Vec::new();
    // 現在開いている <span ...> タグの生文字列（入れ子順）
    let mut open: Vec<&str> = Vec::new();
    // 行頭で開き直すタグ列（= 前の行末時点の open）
    let mut carry: Vec<&str> = Vec::new();
    let mut line = String::new();
    // この行にタグ以外のテキストがあるか（最終改行の後ろに残る
    // 閉じタグだけの断片を「偽の最終行」として捨てるための判定）
    let mut text_seen = false;
    let mut rest = html;
    loop {
        let Some(pos) = rest.find(['<', '\n']) else {
            text_seen |= !rest.is_empty();
            line.push_str(rest);
            break;
        };
        text_seen |= pos > 0;
        line.push_str(&rest[..pos]);
        if rest.as_bytes()[pos] == b'\n' {
            let mut done = String::with_capacity(line.len() + carry.len() * 24);
            for tag in &carry {
                done.push_str(tag);
            }
            done.push_str(&line);
            for _ in &open {
                done.push_str("</span>");
            }
            result.push(done);
            carry.clone_from(&open);
            line.clear();
            text_seen = false;
            rest = &rest[pos + 1..];
        } else {
            // タグ（<span class="..."> か </span>）。テキストはエスケープ済みなので
            // '<' はタグ開始にしか現れない
            let end = match rest[pos..].find('>') {
                Some(i) => pos + i + 1,
                None => rest.len(), // 壊れた入力への保険（打ち切り）
            };
            let tag = &rest[pos..end];
            if tag.starts_with("</") {
                open.pop();
            } else {
                open.push(tag);
            }
            line.push_str(tag);
            rest = &rest[end..];
        }
    }
    if text_seen {
        // 末尾に改行の無い最終行
        let mut done = String::new();
        for tag in &carry {
            done.push_str(tag);
        }
        done.push_str(&line);
        for _ in &open {
            done.push_str("</span>");
        }
        result.push(done);
    }
    result
}

/// 中身が `file: <パス>` の 1 行だけならそのパスを返す（外部ファイル参照の記法）
fn parse_file_ref(trimmed: &str) -> Option<&str> {
    if trimmed.lines().count() != 1 {
        return None;
    }
    let rel = trimmed.strip_prefix("file:")?.trim();
    (!rel.is_empty()).then_some(rel)
}

impl PageCodeRenderer<'_> {
    /// このページでクライアント描画へのフォールバックが発生したか
    /// （= このページに mermaid.js が必要か）
    pub fn mermaid_fallback_occurred(&self) -> bool {
        self.mermaid_fallback.get()
    }

    /// このページで外部ファイル参照（`file:`）を使ったか。
    /// 使ったページは本文キャッシュに保存しない（仕様ファイルの変更を即反映するため）
    pub fn external_deps_used(&self) -> bool {
        self.external_deps.get()
    }

    /// ` ```openapi ` / ` ```jsonschema ` の変換。
    /// 中身が `file: <パス>` の 1 行なら外部ファイル（プロジェクトルート相対）を読む。
    /// 文書内のファイル $ref も同じ読み込み口（[`ProjectSpecFiles`]）を通る
    fn render_apispec(&self, kind: SpecKind, code: &str) -> Option<String> {
        let files = ProjectSpecFiles {
            root: self.shared.apispec_root.as_deref(),
            external_deps: &self.external_deps,
        };
        let trimmed = code.trim();
        match parse_file_ref(trimmed) {
            Some(rel) => match files.read(rel) {
                Ok(text) => Some(apispec::render_spec(kind, &text, Some(rel), &files)),
                Err(message) => {
                    tracing::warn!(file = rel, "{message}");
                    Some(apispec::error_box(&message, code))
                }
            },
            None => Some(apispec::render_spec(kind, code, None, &files)),
        }
    }

    fn render_mermaid(&self, code: &str) -> Option<String> {
        if !self.shared.mermaid_enabled {
            return None;
        }
        if let Some(options_base) = &self.shared.mermaid_ssr_options {
            let mut options = options_base.clone();
            options.id_prefix = format!("tk{}", self.mermaid_counter.get());
            self.mermaid_counter.set(self.mermaid_counter.get() + 1);
            match tankan::render_svg(code, &options) {
                Ok(svg) => {
                    return Some(format!(
                        "<figure class=\"mermaid-ssr\">\n{svg}\n</figure>\n"
                    ));
                }
                Err(e) if e.is_unsupported() => {
                    // 想定内（未対応図種）: 静かにクライアント描画へ
                    tracing::debug!("mermaid SSR 未対応のためクライアント描画へ: {e}");
                    self.mermaid_fallback.set(true);
                }
                Err(e) => {
                    // 構文エラー: 書き間違いの可能性が高いので可視化する
                    tracing::warn!("mermaid の構文エラー（クライアント描画へフォールバック): {e}");
                    self.mermaid_fallback.set(true);
                }
            }
        }
        Some(format!(
            "<pre class=\"mermaid\">{}</pre>\n",
            escape_html(code)
        ))
    }
}

impl CodeBlockRenderer for PageCodeRenderer<'_> {
    // ⚠️ このディスパッチの特別レンダリング言語集合（mermaid / openapi /
    // jsonschema / math）は yuzu_core::is_special_render_lang（検索インデックスの
    // コード除外判定）と同期させること。言語を追加・削除したら両方を更新する
    fn render(&self, lang: Option<&str>, meta: &CodeBlockMeta, code: &str) -> Option<String> {
        // 特別レンダリング言語は表示メタ（title / 行ハイライト / 行番号）を無視する
        if let Some(lang) = lang {
            if lang == "mermaid" {
                return self.render_mermaid(code);
            }
            if lang == "openapi" {
                return self.render_apispec(SpecKind::OpenApi, code);
            }
            if lang == "jsonschema" {
                return self.render_apispec(SpecKind::JsonSchema, code);
            }
            // ```math は comrak の特殊化（<pre><code class="language-math"
            // data-math-style="display">）に任せる。syntect のトークン一致で
            // 偶然ハイライトされると属性が消え、KaTeX が拾えなくなる
            if lang == "math" {
                return None;
            }
        }
        if !self.shared.highlight_enabled {
            return None;
        }
        let line_numbers = meta
            .line_numbers
            .unwrap_or(self.shared.line_numbers_default);
        let highlighted = lang.and_then(|l| self.shared.highlight(l, code));
        // ハイライトできない（言語なし・未知の言語）場合、メタも行番号も無ければ
        // 従来どおりパーサ既定の <pre><code> に任せる。指定があるときだけ
        // エスケープ済みプレーン本文を同じ構造で描画してメタを機能させる
        if highlighted.is_none() && meta.is_empty() && !line_numbers {
            return None;
        }
        let body = highlighted.unwrap_or_else(|| escape_html(code));

        // 行ごとに <span class="line"> で包む（改行は span の中）。
        // 表示はテーマ CSS の display: block が行を作り、コピーボタンの
        // code.textContent には改行がそのまま残る。行番号は CSS カウンタ、
        // 行ハイライトは hl クラスで、どちらもクライアント JS ゼロ
        let mut inner = String::with_capacity(body.len() + body.len() / 2);
        for (i, fragment) in split_lines_balanced(&body).iter().enumerate() {
            if meta.is_highlighted(i + 1) {
                inner.push_str("<span class=\"line hl\">");
            } else {
                inner.push_str("<span class=\"line\">");
            }
            inner.push_str(fragment);
            inner.push('\n');
            inner.push_str("</span>");
        }

        let mut pre_classes = String::from("highlight");
        if line_numbers {
            pre_classes.push_str(" line-numbers");
        }
        // data-lang はテーマ CSS が言語ラベル表示（::before）に使う
        let pre = match lang {
            Some(l) => {
                let l = escape_html(l);
                format!(
                    "<pre class=\"{pre_classes}\" data-lang=\"{l}\"><code class=\"language-{l}\">{inner}</code></pre>\n"
                )
            }
            None => format!("<pre class=\"{pre_classes}\"><code>{inner}</code></pre>\n"),
        };
        Some(match &meta.title {
            Some(title) => format!(
                "<figure class=\"code-block\">\n<figcaption>{}</figcaption>\n{pre}</figure>\n",
                escape_html(title)
            ),
            None => pre,
        })
    }
}

/// HTML エスケープ（テキストノード・属性値用の最小集合）
pub(crate) fn escape_html(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn client_config() -> MermaidConfig {
        MermaidConfig::default()
    }

    fn ssr_config() -> MermaidConfig {
        MermaidConfig {
            enabled: true,
            backend: MermaidBackend::Ssr,
        }
    }

    fn disabled_highlight() -> HighlightConfig {
        HighlightConfig {
            enabled: false,
            ..HighlightConfig::default()
        }
    }

    fn line_numbers_default() -> HighlightConfig {
        HighlightConfig {
            line_numbers: true,
            ..HighlightConfig::default()
        }
    }

    #[test]
    fn client_では_mermaid_はエスケープ済み_pre_になる() {
        let r = SyntectCodeRenderer::new(&HighlightConfig::default(), &client_config());
        let p = r.page_renderer();
        let html = p
            .render(
                Some("mermaid"),
                &CodeBlockMeta::default(),
                "A->>B: <hello>\n",
            )
            .unwrap();
        assert_eq!(
            html,
            "<pre class=\"mermaid\">A-&gt;&gt;B: &lt;hello&gt;\n</pre>\n"
        );
        assert!(!p.mermaid_fallback_occurred(), "client では常に false");
    }

    #[test]
    fn mermaid_無効なら_none() {
        let r = SyntectCodeRenderer::new(
            &HighlightConfig::default(),
            &MermaidConfig {
                enabled: false,
                backend: MermaidBackend::Ssr,
            },
        );
        assert!(
            r.page_renderer()
                .render(Some("mermaid"), &CodeBlockMeta::default(), "graph TD;")
                .is_none()
        );
    }

    #[test]
    fn ssr_では_sequence_が_svg_になる() {
        let r = SyntectCodeRenderer::new(&HighlightConfig::default(), &ssr_config());
        let p = r.page_renderer();
        let html = p
            .render(
                Some("mermaid"),
                &CodeBlockMeta::default(),
                "sequenceDiagram\n    A->>B: こんにちは\n",
            )
            .unwrap();
        assert!(html.starts_with("<figure class=\"mermaid-ssr\">"));
        assert!(html.contains("<svg class=\"tankan"));
        assert!(html.contains("var(--fg, #1f2328)"), "テーマ変数の注入");
        assert!(!p.mermaid_fallback_occurred());
    }

    #[test]
    fn ssr_で_classdef_付き_flowchart_はフォールバックせず色が埋まる() {
        let r = SyntectCodeRenderer::new(&HighlightConfig::default(), &ssr_config());
        let p = r.page_renderer();
        let html = p
            .render(
                Some("mermaid"),
                &CodeBlockMeta::default(),
                "flowchart TD\n    A[開始]:::hot --> B[終了]\n    classDef hot fill:#f96\n",
            )
            .unwrap();
        assert!(
            html.starts_with("<figure class=\"mermaid-ssr\">"),
            "スタイル構文でも SSR される:\n{html}"
        );
        assert!(html.contains("fill:#f96"), "ユーザ指定色が埋まる");
        assert!(!p.mermaid_fallback_occurred());
    }

    #[test]
    fn ssr_で未対応図種はフォールバックして記録される() {
        let r = SyntectCodeRenderer::new(&HighlightConfig::default(), &ssr_config());
        let p = r.page_renderer();
        // journey は未対応図種（mindmap / timeline は Phase 27 で対応済み）
        let html = p
            .render(
                Some("mermaid"),
                &CodeBlockMeta::default(),
                "journey\n    title 一日\n    section 朝\n      起床: 5: 私\n",
            )
            .unwrap();
        assert!(html.starts_with("<pre class=\"mermaid\">"));
        assert!(p.mermaid_fallback_occurred());

        // 新しいページレンダラは初期状態から始まる
        let p = r.page_renderer();
        assert!(!p.mermaid_fallback_occurred());
    }

    #[test]
    fn ssr_では_flowchart_も_svg_になる() {
        let r = SyntectCodeRenderer::new(&HighlightConfig::default(), &ssr_config());
        let p = r.page_renderer();
        let html = p
            .render(
                Some("mermaid"),
                &CodeBlockMeta::default(),
                "flowchart TD\n    A-->B\n",
            )
            .unwrap();
        assert!(html.contains("tankan-flowchart"), "{html}");
        assert!(!p.mermaid_fallback_occurred());
    }

    #[test]
    fn ssr_で構文エラーもフォールバックする() {
        let r = SyntectCodeRenderer::new(&HighlightConfig::default(), &ssr_config());
        let p = r.page_renderer();
        let html = p
            .render(
                Some("mermaid"),
                &CodeBlockMeta::default(),
                "sequenceDiagram\n    矢印のない行\n",
            )
            .unwrap();
        assert!(html.starts_with("<pre class=\"mermaid\">"));
        assert!(p.mermaid_fallback_occurred());
    }

    #[test]
    fn ssr_の_svg_はページ内で_id_が一意になる() {
        let r = SyntectCodeRenderer::new(&HighlightConfig::default(), &ssr_config());
        let p = r.page_renderer();
        let a = p
            .render(
                Some("mermaid"),
                &CodeBlockMeta::default(),
                "sequenceDiagram\nA->>B: x\n",
            )
            .unwrap();
        let b = p
            .render(
                Some("mermaid"),
                &CodeBlockMeta::default(),
                "sequenceDiagram\nC->>D: y\n",
            )
            .unwrap();
        assert!(a.contains(r#"id="tk0-head""#));
        assert!(b.contains(r#"id="tk1-head""#));
    }

    #[test]
    fn rust_はハイライトされ_css_クラスが付く() {
        let r = SyntectCodeRenderer::new(&HighlightConfig::default(), &client_config());
        let p = r.page_renderer();
        let html = p
            .render(Some("rust"), &CodeBlockMeta::default(), "fn main() {}\n")
            .unwrap();
        assert!(html.starts_with("<pre class=\"highlight\" data-lang=\"rust\">"));
        assert!(
            html.contains("class=\"yz-"),
            "yz- 接頭辞のクラス出力: {html}"
        );
        assert!(!html.contains("style="), "インラインスタイル禁止: {html}");
    }

    #[test]
    fn 未知の言語と言語なしは_none() {
        let r = SyntectCodeRenderer::new(&HighlightConfig::default(), &client_config());
        let p = r.page_renderer();
        assert!(
            p.render(Some("unknown-lang-xyz"), &CodeBlockMeta::default(), "x")
                .is_none()
        );
        assert!(p.render(None, &CodeBlockMeta::default(), "x").is_none());
    }

    #[test]
    fn math_はハイライトせず_comrak_の特殊化に任せる() {
        let r = SyntectCodeRenderer::new(&HighlightConfig::default(), &client_config());
        assert!(
            r.page_renderer()
                .render(Some("math"), &CodeBlockMeta::default(), "x^2")
                .is_none()
        );
    }

    #[test]
    fn ハイライト無効なら_none() {
        let r = SyntectCodeRenderer::new(&disabled_highlight(), &client_config());
        assert!(
            r.page_renderer()
                .render(Some("rust"), &CodeBlockMeta::default(), "fn main() {}")
                .is_none()
        );
    }

    #[test]
    fn openapi_の_file_参照はルート相対で読み外部依存を記録する() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("specs")).unwrap();
        std::fs::write(dir.path().join("specs/api.yaml"), "type: object\n").unwrap();

        let mut r = SyntectCodeRenderer::new(&HighlightConfig::default(), &client_config());
        r.set_project_root(dir.path().to_path_buf());
        let p = r.page_renderer();
        assert!(!p.external_deps_used());

        let html = p
            .render(
                Some("jsonschema"),
                &CodeBlockMeta::default(),
                "file: specs/api.yaml\n",
            )
            .unwrap();
        assert!(p.external_deps_used(), "file: 参照で外部依存が立つ");
        // スタブ実装でも「レンダラを通った」ことは HTML の存在で確認できる
        assert!(!html.is_empty());

        // 新しいページレンダラは初期状態から始まる
        let p = r.page_renderer();
        assert!(!p.external_deps_used());
    }

    #[test]
    fn openapi_の_file_参照はルート外と不在を拒否する() {
        let dir = tempfile::tempdir().unwrap();
        let mut r = SyntectCodeRenderer::new(&HighlightConfig::default(), &client_config());
        r.set_project_root(dir.path().to_path_buf());
        let p = r.page_renderer();

        // ルート外（.. 逸脱）
        let html = p
            .render(
                Some("openapi"),
                &CodeBlockMeta::default(),
                "file: ../outside.yaml\n",
            )
            .unwrap();
        assert!(html.contains("markdown-alert-caution"), "{html}");

        // 不在ファイル
        let html = p
            .render(
                Some("openapi"),
                &CodeBlockMeta::default(),
                "file: missing.yaml\n",
            )
            .unwrap();
        assert!(html.contains("markdown-alert-caution"), "{html}");
    }

    #[test]
    fn インラインの_openapi_は外部依存を立てない() {
        let r = SyntectCodeRenderer::new(&HighlightConfig::default(), &client_config());
        let p = r.page_renderer();
        let _ = p
            .render(
                Some("openapi"),
                &CodeBlockMeta::default(),
                "openapi: 3.0.3\n",
            )
            .unwrap();
        assert!(!p.external_deps_used(), "インラインはキャッシュ可能");
    }

    #[test]
    fn file_参照の判定は一行のみ() {
        assert_eq!(super::parse_file_ref("file: a.yaml"), Some("a.yaml"));
        assert_eq!(super::parse_file_ref("file:a.yaml"), Some("a.yaml"));
        assert_eq!(super::parse_file_ref("file:"), None);
        assert_eq!(
            super::parse_file_ref("file: a.yaml\nx: 1"),
            None,
            "複数行はインライン扱い"
        );
        assert_eq!(super::parse_file_ref("openapi: 3.0.3"), None);
    }

    #[test]
    fn インラインブロック内のファイル_ref_も外部依存を立てる() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("specs")).unwrap();
        std::fs::write(
            dir.path().join("specs/common.yaml"),
            "components:\n  schemas:\n    User:\n      type: object\n      properties:\n        id:\n          type: string\n",
        )
        .unwrap();

        let mut r = SyntectCodeRenderer::new(&HighlightConfig::default(), &client_config());
        r.set_project_root(dir.path().to_path_buf());
        let p = r.page_renderer();

        let src = concat!(
            "openapi: 3.0.3\n",
            "info:\n  title: T\n  version: \"1\"\n",
            "paths:\n  /u:\n    get:\n      responses:\n        \"200\":\n",
            "          description: ok\n          content:\n            application/json:\n",
            "              schema:\n",
            "                $ref: \"specs/common.yaml#/components/schemas/User\"\n",
        );
        let html = p
            .render(Some("openapi"), &CodeBlockMeta::default(), src)
            .unwrap();
        assert!(
            p.external_deps_used(),
            "文書内のファイル $ref でもキャッシュ非対象の印が立つ"
        );
        assert!(html.contains("<code>id</code>"), "{html}");
    }

    // --- Phase 39: 表示メタ（title / 行ハイライト / 行番号） ---

    fn meta(info: &str) -> CodeBlockMeta {
        // 描画テストではパース済みメタを直接組み立てず、実際の情報文字列経由の
        // 値を使いたいが、パーサは yuzu-core 側なのでここでは手組みする
        let mut m = CodeBlockMeta::default();
        for token in info.split_whitespace() {
            match token {
                "showLineNumbers" => m.line_numbers = Some(true),
                "noLineNumbers" => m.line_numbers = Some(false),
                t if t.starts_with("title=") => {
                    m.title = Some(t.trim_start_matches("title=").trim_matches('"').to_string());
                }
                t if t.starts_with('{') => {
                    for part in t.trim_matches(['{', '}']).split(',') {
                        match part.split_once('-') {
                            Some((s, e)) => m
                                .highlight_lines
                                .push((s.parse().unwrap(), e.parse().unwrap())),
                            None => {
                                let n = part.parse().unwrap();
                                m.highlight_lines.push((n, n));
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        m
    }

    #[test]
    fn 全コードブロックは行_span_で包まれる() {
        let r = SyntectCodeRenderer::new(&HighlightConfig::default(), &client_config());
        let p = r.page_renderer();
        let html = p
            .render(
                Some("rust"),
                &CodeBlockMeta::default(),
                "fn main() {\n    println!(\"hi\");\n}\n",
            )
            .unwrap();
        assert_eq!(html.matches("<span class=\"line\">").count(), 3, "{html}");
        assert!(!html.contains("line hl"));
        assert!(!html.contains("line-numbers"));
    }

    #[test]
    fn title_は_figure_と_figcaption_になる() {
        let r = SyntectCodeRenderer::new(&HighlightConfig::default(), &client_config());
        let p = r.page_renderer();
        let html = p
            .render(
                Some("rust"),
                &meta(r#"title="src/main.rs""#),
                "fn main() {}\n",
            )
            .unwrap();
        assert!(html.starts_with("<figure class=\"code-block\">"), "{html}");
        assert!(html.contains("<figcaption>src/main.rs</figcaption>"));
        assert!(html.trim_end().ends_with("</figure>"));
    }

    #[test]
    fn title_はエスケープされる() {
        let r = SyntectCodeRenderer::new(&HighlightConfig::default(), &client_config());
        let p = r.page_renderer();
        let html = p
            .render(
                Some("rust"),
                &meta(r#"title="<script>alert(1)</script>""#),
                "fn main() {}\n",
            )
            .unwrap();
        assert!(
            html.contains("<figcaption>&lt;script&gt;alert(1)&lt;/script&gt;</figcaption>"),
            "{html}"
        );
    }

    #[test]
    fn 行ハイライトは指定行だけ_hl_クラスになる() {
        let r = SyntectCodeRenderer::new(&HighlightConfig::default(), &client_config());
        let p = r.page_renderer();
        let html = p
            .render(
                Some("rust"),
                &meta("{2}"),
                "fn a() {}\nfn b() {}\nfn c() {}\n",
            )
            .unwrap();
        let lines: Vec<&str> = html.split("<span class=\"line").skip(1).collect();
        assert_eq!(lines.len(), 3);
        assert!(!lines[0].starts_with(" hl"), "1 行目は非対象");
        assert!(lines[1].starts_with(" hl\">"), "2 行目が対象: {html}");
        assert!(!lines[2].starts_with(" hl"), "3 行目は非対象");
    }

    #[test]
    fn 行番号はブロック指定が設定より優先される() {
        // 設定 off + showLineNumbers → on
        let r = SyntectCodeRenderer::new(&HighlightConfig::default(), &client_config());
        let html = r
            .page_renderer()
            .render(Some("rust"), &meta("showLineNumbers"), "fn main() {}\n")
            .unwrap();
        assert!(html.contains("class=\"highlight line-numbers\""), "{html}");

        // 設定 on（既定）→ on
        let r = SyntectCodeRenderer::new(&line_numbers_default(), &client_config());
        let html = r
            .page_renderer()
            .render(Some("rust"), &CodeBlockMeta::default(), "fn main() {}\n")
            .unwrap();
        assert!(html.contains("line-numbers"));

        // 設定 on + noLineNumbers → off
        let html = r
            .page_renderer()
            .render(Some("rust"), &meta("noLineNumbers"), "fn main() {}\n")
            .unwrap();
        assert!(!html.contains("line-numbers"), "{html}");
    }

    #[test]
    fn 未知の言語でもメタがあればプレーン描画される() {
        let r = SyntectCodeRenderer::new(&HighlightConfig::default(), &client_config());
        let p = r.page_renderer();
        let html = p
            .render(
                Some("unknown-lang-xyz"),
                &meta(r#"title="出力例" {1}"#),
                "<a> & b\nplain\n",
            )
            .unwrap();
        assert!(html.contains("<figcaption>出力例</figcaption>"), "{html}");
        assert!(html.contains("&lt;a&gt; &amp; b"), "エスケープ済み: {html}");
        assert!(html.contains("<span class=\"line hl\">"));
        assert!(html.contains("data-lang=\"unknown-lang-xyz\""));
        assert!(!html.contains("class=\"yz-"), "syntect クラスは付かない");
    }

    #[test]
    fn 言語なしでもメタがあればプレーン描画される() {
        let r = SyntectCodeRenderer::new(&HighlightConfig::default(), &client_config());
        let p = r.page_renderer();
        let html = p.render(None, &meta("showLineNumbers"), "a\nb\n").unwrap();
        assert!(
            html.starts_with("<pre class=\"highlight line-numbers\"><code>"),
            "{html}"
        );
        assert!(!html.contains("data-lang"));
    }

    #[test]
    fn 特別レンダリング言語はメタを無視する() {
        let r = SyntectCodeRenderer::new(&HighlightConfig::default(), &client_config());
        let p = r.page_renderer();
        let html = p
            .render(Some("mermaid"), &meta(r#"title="図""#), "A->>B: x\n")
            .unwrap();
        assert!(html.starts_with("<pre class=\"mermaid\">"), "{html}");
        assert!(!html.contains("figcaption"));

        // math も従来どおり None（comrak の特殊化に任せる）
        assert!(p.render(Some("math"), &meta("{1}"), "x^2").is_none());
    }

    #[test]
    fn ハイライト無効ならメタがあっても_none() {
        let r = SyntectCodeRenderer::new(&disabled_highlight(), &client_config());
        assert!(
            r.page_renderer()
                .render(Some("rust"), &meta(r#"title="x""#), "fn main() {}")
                .is_none()
        );
    }

    #[test]
    fn 行またぎのスコープでも行_span_はバランスする() {
        // Rust の raw 文字列リテラルは複数行で 1 スコープになる
        let r = SyntectCodeRenderer::new(&HighlightConfig::default(), &client_config());
        let p = r.page_renderer();
        let html = p
            .render(
                Some("rust"),
                &CodeBlockMeta::default(),
                "let s = r#\"one\ntwo\nthree\"#;\n",
            )
            .unwrap();
        // 各行 span 内で開きタグと閉じタグの数が一致する（自己完結）
        for fragment in html.split("<span class=\"line\">").skip(1) {
            let fragment = fragment.split("\n</span>").next().unwrap();
            assert_eq!(
                fragment.matches("<span").count(),
                fragment.matches("</span>").count(),
                "行内で span がバランスする: {fragment}"
            );
        }
    }

    #[test]
    fn split_lines_balanced_の分割規則() {
        // 行またぎ span: 行末で閉じて次行頭で開き直す
        let lines = split_lines_balanced("<span class=\"a\">one\ntwo</span>\n");
        assert_eq!(
            lines,
            vec![
                "<span class=\"a\">one</span>".to_string(),
                "<span class=\"a\">two</span>".to_string(),
            ]
        );
        // 末尾改行なしの最終行・空行
        assert_eq!(split_lines_balanced("a\n\nb"), vec!["a", "", "b"]);
        // 末尾改行の後には行を作らない
        assert_eq!(split_lines_balanced("a\n"), vec!["a"]);
    }
}
