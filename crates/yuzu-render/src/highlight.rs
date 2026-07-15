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

use yuzu_config::{MermaidBackend, MermaidConfig};
use yuzu_core::CodeBlockRenderer;

use crate::apispec::{self, SpecFiles, SpecKind};

/// syntect のクラス名接頭辞。テーマ CSS のクラスと衝突しないようにする。
/// `css.rs` の CSS 生成と必ず同じ値を使うこと
pub(crate) const CLASS_STYLE: ClassStyle = ClassStyle::SpacedPrefixed { prefix: "yz-" };

/// [`CodeBlockRenderer`] の実装。
/// - `lang == "mermaid"` → SSR（tankan）またはクライアント描画へ
/// - 既知の言語 → syntect ハイライト（CSS クラス）
/// - 言語なし・未知の言語 → `None`（パーサ既定のエスケープ済み `<pre><code>`）
pub struct SyntectCodeRenderer {
    syntax_set: SyntaxSet,
    highlight_enabled: bool,
    mermaid_enabled: bool,
    /// backend == Ssr のときだけ Some
    mermaid_ssr: Option<MermaidSsr>,
    /// OpenAPI / JSON Schema ブロックのページ単位状態
    apispec: ApiSpecState,
}

/// ` ```openapi ` / ` ```jsonschema ` のページ単位状態（Cell パターンは MermaidSsr と同じ）
#[derive(Default)]
struct ApiSpecState {
    /// `file:` 参照の基準ディレクトリ（プロジェクトルート）。
    /// 未設定（単体テスト等）ではファイル参照をエラーボックスにする
    root: Option<PathBuf>,
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

/// SSR のページ単位状態。
/// `render_site` はページを直列に処理し `CodeBlockRenderer` は同一スレッドの
/// `&self` 呼び出しのみなので、interior mutability は `Cell` で足りる
///（将来ページ並列化する場合はページごとに状態を分離する設計変更が必要）
struct MermaidSsr {
    options_base: tankan::Options,
    /// このページでクライアント描画へのフォールバックが発生したか
    fallback: Cell<bool>,
    /// ページ内の SVG 連番（`<marker>` id の一意化用）
    counter: Cell<usize>,
}

impl SyntectCodeRenderer {
    pub fn new(highlight_enabled: bool, mermaid: &MermaidConfig) -> Self {
        let mermaid_ssr =
            (mermaid.enabled && mermaid.backend == MermaidBackend::Ssr).then(|| MermaidSsr {
                options_base: tankan::Options {
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
                },
                fallback: Cell::new(false),
                counter: Cell::new(0),
            });
        Self {
            // ClassedHTMLGenerator の行単位 API は newlines 版とセットで使う。
            // syntect デフォルトには TypeScript/TSX/TOML/Dockerfile 等がないため、
            // bat のアセット由来の拡張セット（two-face。デフォルト構文も内包）を使う
            syntax_set: two_face::syntax::extra_newlines(),
            highlight_enabled,
            mermaid_enabled: mermaid.enabled,
            mermaid_ssr,
            apispec: ApiSpecState::default(),
        }
    }

    /// `file:` 参照の基準ディレクトリ（プロジェクトルート）を設定する
    pub fn set_project_root(&mut self, root: PathBuf) {
        self.apispec.root = Some(root);
    }

    /// ページ処理の直前に呼ぶ（SSR のページ単位状態をリセット）
    pub fn begin_page(&self) {
        if let Some(ssr) = &self.mermaid_ssr {
            ssr.fallback.set(false);
            ssr.counter.set(0);
        }
        self.apispec.external_deps.set(false);
    }

    /// 直前ページでクライアント描画へのフォールバックが発生したか
    /// （= このページに mermaid.js が必要か）
    pub fn mermaid_fallback_occurred(&self) -> bool {
        self.mermaid_ssr
            .as_ref()
            .is_some_and(|ssr| ssr.fallback.get())
    }

    /// 直前ページで外部ファイル参照（`file:`）を使ったか。
    /// 使ったページは本文キャッシュに保存しない（仕様ファイルの変更を即反映するため）
    pub fn external_deps_used(&self) -> bool {
        self.apispec.external_deps.get()
    }

    /// ` ```openapi ` / ` ```jsonschema ` の変換。
    /// 中身が `file: <パス>` の 1 行なら外部ファイル（プロジェクトルート相対）を読む。
    /// 文書内のファイル $ref も同じ読み込み口（[`ProjectSpecFiles`]）を通る
    fn render_apispec(&self, kind: SpecKind, code: &str) -> Option<String> {
        let files = ProjectSpecFiles {
            root: self.apispec.root.as_deref(),
            external_deps: &self.apispec.external_deps,
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
        if !self.mermaid_enabled {
            return None;
        }
        if let Some(ssr) = &self.mermaid_ssr {
            let mut options = ssr.options_base.clone();
            options.id_prefix = format!("tk{}", ssr.counter.get());
            ssr.counter.set(ssr.counter.get() + 1);
            match tankan::render_svg(code, &options) {
                Ok(svg) => {
                    return Some(format!(
                        "<figure class=\"mermaid-ssr\">\n{svg}\n</figure>\n"
                    ));
                }
                Err(e) if e.is_unsupported() => {
                    // 想定内（未対応図種）: 静かにクライアント描画へ
                    tracing::debug!("mermaid SSR 未対応のためクライアント描画へ: {e}");
                    ssr.fallback.set(true);
                }
                Err(e) => {
                    // 構文エラー: 書き間違いの可能性が高いので可視化する
                    tracing::warn!("mermaid の構文エラー（クライアント描画へフォールバック): {e}");
                    ssr.fallback.set(true);
                }
            }
        }
        Some(format!(
            "<pre class=\"mermaid\">{}</pre>\n",
            escape_html(code)
        ))
    }

    fn highlight(&self, lang: &str, code: &str) -> Option<String> {
        if !self.highlight_enabled {
            return None;
        }
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

/// 中身が `file: <パス>` の 1 行だけならそのパスを返す（外部ファイル参照の記法）
fn parse_file_ref(trimmed: &str) -> Option<&str> {
    if trimmed.lines().count() != 1 {
        return None;
    }
    let rel = trimmed.strip_prefix("file:")?.trim();
    (!rel.is_empty()).then_some(rel)
}

impl CodeBlockRenderer for SyntectCodeRenderer {
    fn render(&self, lang: Option<&str>, code: &str) -> Option<String> {
        let lang = lang?;
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
        let inner = self.highlight(lang, code)?;
        // data-lang はテーマ CSS が言語ラベル表示（::before）に使う
        Some(format!(
            "<pre class=\"highlight\" data-lang=\"{lang}\"><code class=\"language-{lang}\">{}</code></pre>\n",
            inner,
            lang = escape_html(lang),
        ))
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

    #[test]
    fn client_では_mermaid_はエスケープ済み_pre_になる() {
        let r = SyntectCodeRenderer::new(true, &client_config());
        let html = r.render(Some("mermaid"), "A->>B: <hello>\n").unwrap();
        assert_eq!(
            html,
            "<pre class=\"mermaid\">A-&gt;&gt;B: &lt;hello&gt;\n</pre>\n"
        );
        assert!(!r.mermaid_fallback_occurred(), "client では常に false");
    }

    #[test]
    fn mermaid_無効なら_none() {
        let r = SyntectCodeRenderer::new(
            true,
            &MermaidConfig {
                enabled: false,
                backend: MermaidBackend::Ssr,
            },
        );
        assert!(r.render(Some("mermaid"), "graph TD;").is_none());
    }

    #[test]
    fn ssr_では_sequence_が_svg_になる() {
        let r = SyntectCodeRenderer::new(true, &ssr_config());
        r.begin_page();
        let html = r
            .render(Some("mermaid"), "sequenceDiagram\n    A->>B: こんにちは\n")
            .unwrap();
        assert!(html.starts_with("<figure class=\"mermaid-ssr\">"));
        assert!(html.contains("<svg class=\"tankan"));
        assert!(html.contains("var(--fg, #1f2328)"), "テーマ変数の注入");
        assert!(!r.mermaid_fallback_occurred());
    }

    #[test]
    fn ssr_で_classdef_付き_flowchart_はフォールバックせず色が埋まる() {
        let r = SyntectCodeRenderer::new(true, &ssr_config());
        r.begin_page();
        let html = r
            .render(
                Some("mermaid"),
                "flowchart TD\n    A[開始]:::hot --> B[終了]\n    classDef hot fill:#f96\n",
            )
            .unwrap();
        assert!(
            html.starts_with("<figure class=\"mermaid-ssr\">"),
            "スタイル構文でも SSR される:\n{html}"
        );
        assert!(html.contains("fill:#f96"), "ユーザ指定色が埋まる");
        assert!(!r.mermaid_fallback_occurred());
    }

    #[test]
    fn ssr_で未対応図種はフォールバックして記録される() {
        let r = SyntectCodeRenderer::new(true, &ssr_config());
        r.begin_page();
        // mindmap は未対応図種（classDiagram / pie は Phase 16 で対応済み）
        let html = r
            .render(Some("mermaid"), "mindmap\n  root((中心))\n")
            .unwrap();
        assert!(html.starts_with("<pre class=\"mermaid\">"));
        assert!(r.mermaid_fallback_occurred());

        // begin_page でリセットされる
        r.begin_page();
        assert!(!r.mermaid_fallback_occurred());
    }

    #[test]
    fn ssr_では_flowchart_も_svg_になる() {
        let r = SyntectCodeRenderer::new(true, &ssr_config());
        r.begin_page();
        let html = r
            .render(Some("mermaid"), "flowchart TD\n    A-->B\n")
            .unwrap();
        assert!(html.contains("tankan-flowchart"), "{html}");
        assert!(!r.mermaid_fallback_occurred());
    }

    #[test]
    fn ssr_で構文エラーもフォールバックする() {
        let r = SyntectCodeRenderer::new(true, &ssr_config());
        r.begin_page();
        let html = r
            .render(Some("mermaid"), "sequenceDiagram\n    矢印のない行\n")
            .unwrap();
        assert!(html.starts_with("<pre class=\"mermaid\">"));
        assert!(r.mermaid_fallback_occurred());
    }

    #[test]
    fn ssr_の_svg_はページ内で_id_が一意になる() {
        let r = SyntectCodeRenderer::new(true, &ssr_config());
        r.begin_page();
        let a = r
            .render(Some("mermaid"), "sequenceDiagram\nA->>B: x\n")
            .unwrap();
        let b = r
            .render(Some("mermaid"), "sequenceDiagram\nC->>D: y\n")
            .unwrap();
        assert!(a.contains(r#"id="tk0-head""#));
        assert!(b.contains(r#"id="tk1-head""#));
    }

    #[test]
    fn rust_はハイライトされ_css_クラスが付く() {
        let r = SyntectCodeRenderer::new(true, &client_config());
        let html = r.render(Some("rust"), "fn main() {}\n").unwrap();
        assert!(html.starts_with("<pre class=\"highlight\" data-lang=\"rust\">"));
        assert!(
            html.contains("class=\"yz-"),
            "yz- 接頭辞のクラス出力: {html}"
        );
        assert!(!html.contains("style="), "インラインスタイル禁止: {html}");
    }

    #[test]
    fn 未知の言語と言語なしは_none() {
        let r = SyntectCodeRenderer::new(true, &client_config());
        assert!(r.render(Some("unknown-lang-xyz"), "x").is_none());
        assert!(r.render(None, "x").is_none());
    }

    #[test]
    fn math_はハイライトせず_comrak_の特殊化に任せる() {
        let r = SyntectCodeRenderer::new(true, &client_config());
        assert!(r.render(Some("math"), "x^2").is_none());
    }

    #[test]
    fn ハイライト無効なら_none() {
        let r = SyntectCodeRenderer::new(false, &client_config());
        assert!(r.render(Some("rust"), "fn main() {}").is_none());
    }

    #[test]
    fn openapi_の_file_参照はルート相対で読み外部依存を記録する() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("specs")).unwrap();
        std::fs::write(dir.path().join("specs/api.yaml"), "type: object\n").unwrap();

        let mut r = SyntectCodeRenderer::new(true, &client_config());
        r.set_project_root(dir.path().to_path_buf());
        r.begin_page();
        assert!(!r.external_deps_used());

        let html = r
            .render(Some("jsonschema"), "file: specs/api.yaml\n")
            .unwrap();
        assert!(r.external_deps_used(), "file: 参照で外部依存が立つ");
        // スタブ実装でも「レンダラを通った」ことは HTML の存在で確認できる
        assert!(!html.is_empty());

        // begin_page でリセットされる
        r.begin_page();
        assert!(!r.external_deps_used());
    }

    #[test]
    fn openapi_の_file_参照はルート外と不在を拒否する() {
        let dir = tempfile::tempdir().unwrap();
        let mut r = SyntectCodeRenderer::new(true, &client_config());
        r.set_project_root(dir.path().to_path_buf());
        r.begin_page();

        // ルート外（.. 逸脱）
        let html = r
            .render(Some("openapi"), "file: ../outside.yaml\n")
            .unwrap();
        assert!(html.contains("markdown-alert-caution"), "{html}");

        // 不在ファイル
        let html = r.render(Some("openapi"), "file: missing.yaml\n").unwrap();
        assert!(html.contains("markdown-alert-caution"), "{html}");
    }

    #[test]
    fn インラインの_openapi_は外部依存を立てない() {
        let r = SyntectCodeRenderer::new(true, &client_config());
        r.begin_page();
        let _ = r.render(Some("openapi"), "openapi: 3.0.3\n").unwrap();
        assert!(!r.external_deps_used(), "インラインはキャッシュ可能");
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

        let mut r = SyntectCodeRenderer::new(true, &client_config());
        r.set_project_root(dir.path().to_path_buf());
        r.begin_page();

        let src = concat!(
            "openapi: 3.0.3\n",
            "info:\n  title: T\n  version: \"1\"\n",
            "paths:\n  /u:\n    get:\n      responses:\n        \"200\":\n",
            "          description: ok\n          content:\n            application/json:\n",
            "              schema:\n",
            "                $ref: \"specs/common.yaml#/components/schemas/User\"\n",
        );
        let html = r.render(Some("openapi"), src).unwrap();
        assert!(
            r.external_deps_used(),
            "文書内のファイル $ref でもキャッシュ非対象の印が立つ"
        );
        assert!(html.contains("<code>id</code>"), "{html}");
    }
}
