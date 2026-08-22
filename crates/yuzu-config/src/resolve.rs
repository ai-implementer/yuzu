use std::fs;
use std::path::{Component, Path, PathBuf};

use kabosu::{
    DecodeOptions, DiagnosticCode, Document, ParseError, ParseErrorKind, UnknownKeys,
    UnsupportedFeature, ValueKind,
};

use crate::error::ConfigIssue;
use crate::{CONFIG_FILE_NAME, Config, ConfigError};

/// ユーザテーマディレクトリ名（プロジェクトルート直下）
const THEME_DIR_NAME: &str = "theme";
/// 静的物パススルーのディレクトリ名（プロジェクトルート直下）
const PUBLIC_DIR_NAME: &str = "public";
/// ツール管理ディレクトリ名
const YUZU_DIR_NAME: &str = ".yuzu";

/// デフォルトをマージし、パスと base_url を解決した設定
#[derive(Debug, Clone)]
pub struct ResolvedConfig {
    pub config: Config,
    /// プロジェクトルート（`yuzu.toml` のあるディレクトリ）
    pub root: PathBuf,
    pub content_dir: PathBuf,
    pub output_dir: PathBuf,
    /// プロジェクトの `theme/` が存在する場合のみ Some（埋め込みテーマの上書き元）
    pub theme_dir: Option<PathBuf>,
    /// `public/` が存在する場合のみ Some
    pub public_dir: Option<PathBuf>,
    /// `build.base_url` ?? `site.base_url` ?? "/" を正規化したもの。
    /// パス形は常に先頭・末尾スラッシュ付き（`/` または `/docs/`）
    pub base_url: String,
    /// 設定ファイル自体の警告（読み込みは成功するが注意が要るもの）。
    /// `yuzu lint` / `check` が診断として報告し、他コマンドは読み込み時の警告で済ませる
    /// （yuzu-config はログを出さないので、表示は呼び出し側の責務）
    pub diagnostics: Vec<ConfigDiagnostic>,
}

/// プロジェクトルートの `yuzu.toml` を読み込み、解決済み設定を返す
pub fn load(root: &Path) -> Result<ResolvedConfig, ConfigError> {
    let path = root.join(CONFIG_FILE_NAME);
    let text = fs::read_to_string(&path).map_err(|source| ConfigError::Io {
        path: path.clone(),
        source,
    })?;
    let (config, doc) = parse_config(&text, &path)?;
    let mut diagnostics = Vec::new();

    // 出力ディレクトリの境界検証。`Path::join` は絶対パス引数で左辺を捨て `..` も
    // 潰さないため、無検証だと output.clean の remove_dir_all がルート外・ルート自身へ
    // 届く。ここが output.dir → output_dir の唯一の変換点なので、全コマンドを覆える
    let output_dir = resolve_dir_setting(root, &config.output.dir).map_err(|issue| {
        ConfigError::UnsafeOutputDir {
            key: "output.dir",
            value: config.output.dir.clone(),
            reason: issue.reason(),
            root: root.to_path_buf(),
        }
    })?;
    // 入力側のディレクトリを output.clean の再帰削除から守る。
    //
    // ⚠️ 比較は 3 点そろって初めて正しい。どれか 1 つでも欠けるとすり抜ける:
    //   1. **字句正規化してから比較する** — `input.dir = "a/../dist/content"` は
    //      `root.join()` したままだと `dist` の前方一致にならない
    //   2. **双方向で判定する** — 片方向だけだと `output.dir = "content/sub"`
    //      （保護対象の子）を取りこぼす
    //   3. **input.dir 以外も対象にする** — public / theme も原本であって生成物ではない
    for (label, guard) in [
        ("input.dir", root.join(&config.input.dir)),
        ("public/", root.join(PUBLIC_DIR_NAME)),
        ("theme/", root.join(THEME_DIR_NAME)),
        (".yuzu", root.join(YUZU_DIR_NAME)),
    ] {
        let guard = lexically_normalize(&guard);
        if guard.starts_with(&output_dir) || output_dir.starts_with(&guard) {
            return Err(ConfigError::UnsafeOutputDir {
                key: "output.dir",
                value: config.output.dir.clone(),
                reason: match label {
                    "input.dir" => "原稿ディレクトリ（input.dir）と重なっています",
                    "public/" => "public/（静的物の原本）と重なっています",
                    "theme/" => "theme/（テーマの原本）と重なっています",
                    _ => "ツール管理ディレクトリ（.yuzu）と重なっています",
                },
                root: root.to_path_buf(),
            });
        }
    }

    // input.dir は読むだけで削除経路がないため警告に留める（モノレポで原稿を
    // 共有する運用を即死させない）。ただし診断のパス表示と ignore glob の評価は
    // content_dir 相対を前提にしているので、黙って通すこともしない
    let content_dir = match resolve_dir_setting(root, &config.input.dir) {
        Ok(dir) => dir,
        Err(issue) => {
            let (line, col) = key_position(&doc, &["input", "dir"]).unwrap_or((1, 1));
            diagnostics.push(ConfigDiagnostic {
                rule: RULE_PATH_OUTSIDE_ROOT,
                key_path: "input.dir".to_string(),
                line,
                col,
                message: format!(
                    "input.dir `{}` はプロジェクトルート配下ではありません（{}）。診断のパス表示と input.ignore の glob 評価が想定外になります",
                    config.input.dir,
                    issue.reason()
                ),
            });
            root.join(&config.input.dir)
        }
    };

    let base_url = normalize_base_url(
        config
            .build
            .base_url
            .as_deref()
            .or(config.site.base_url.as_deref())
            .unwrap_or("/"),
    );

    let theme_dir = Some(root.join(THEME_DIR_NAME)).filter(|p| p.is_dir());
    let public_dir = Some(root.join(PUBLIC_DIR_NAME)).filter(|p| p.is_dir());

    Ok(ResolvedConfig {
        content_dir,
        output_dir,
        theme_dir,
        public_dir,
        base_url,
        root: root.to_path_buf(),
        config,
        diagnostics,
    })
}

/// `yuzu.toml` の本文を [`Config`] へ変換する（`load` の I/O を伴わない部分）。
///
/// - 構文エラーは最初の 1 件で `Syntax`（重複キー・未対応構文もここ）
/// - 型不一致・未知キー（Deny）・不正値は全件蓄積して `Invalid`
///
/// `path` はエラー表示用。返す `Document` は span → 行列の変換に使う
pub(crate) fn parse_config(text: &str, path: &Path) -> Result<(Config, Document), ConfigError> {
    let doc = Document::parse(text).map_err(|e| {
        let (line, col) = line_col(text, e.span().start);
        ConfigError::Syntax {
            path: path.to_path_buf(),
            line,
            col,
            message: syntax_message(text, &e),
        }
    })?;

    // 未知キーは設定エラーにする（kabosu.md「yuzu への統合」）。タイポ・旧形式の
    // camelCase キーが黙って無視されて「設定したのに効かない」事故を防ぐ
    let mut options = DecodeOptions::default();
    options.unknown_keys = UnknownKeys::Deny;
    let report = kabosu::decode::<Config>(&doc, options);
    if report.has_errors() {
        return Err(ConfigError::Invalid {
            path: path.to_path_buf(),
            issues: report
                .diagnostics()
                .iter()
                .map(|d| issue_of(&doc, d))
                .collect(),
        });
    }
    // Deny では警告が残らない（省略通知も Error に伴ってしか出ない）ので、
    // エラーなし = 値あり
    let (value, _) = report.into_parts();
    let config = value.expect("エラーなしなら decode の値がある");
    Ok((config, doc))
}

/// kabosu の診断（英語・構造化）を日本語の位置付き 1 件へ写す
fn issue_of(doc: &Document, d: &kabosu::Diagnostic) -> ConfigIssue {
    let (line, col) = line_col(doc.source(), d.span().start);
    let key_path = d.key_path().to_string();
    let message = match d.code() {
        DiagnosticCode::TypeMismatch { expected, found } => format!(
            "`{key_path}` の型が違います（期待: {}、実際: {}）",
            kind_name(*expected),
            kind_name(*found)
        ),
        DiagnosticCode::IntegerOutOfRange => {
            format!("`{key_path}` の整数が範囲外です（{}）", d.message())
        }
        DiagnosticCode::MissingKey => format!("`{key_path}` は必須です"),
        DiagnosticCode::UnknownKey { known_keys } => format!(
            "未知のキー `{key_path}` があります（この階層の対応キー: {}）",
            known_keys.join(", ")
        ),
        DiagnosticCode::TooManyDiagnostics { omitted } => {
            format!("ほか {omitted} 件の問題を省略しました")
        }
        // 独自診断（codec.rs）は日本語で組み立て済み。将来の種別もそのまま出す
        _ => d.message().to_string(),
    };
    ConfigIssue {
        key_path,
        line,
        col,
        message,
    }
}

/// 値種別の日本語名（型不一致の文言用）
fn kind_name(kind: ValueKind) -> &'static str {
    match kind {
        ValueKind::String => "文字列",
        ValueKind::Integer => "整数",
        ValueKind::Boolean => "真偽値",
        ValueKind::Array => "配列",
        ValueKind::Table => "テーブル",
        _ => "値",
    }
}

/// 構文エラーの日本語文言
fn syntax_message(text: &str, e: &ParseError) -> String {
    let previous = e
        .previous_span()
        .map(|s| {
            let (line, col) = line_col(text, s.start);
            format!("（先の定義: {line}:{col}）")
        })
        .unwrap_or_default();
    match e.kind() {
        ParseErrorKind::DuplicateKey => {
            format!("キーが重複しています{previous}。TOML では同じキーを 2 回書けません")
        }
        ParseErrorKind::TableConflict => {
            format!("テーブルの定義が既存のキーまたはテーブルと衝突しています{previous}")
        }
        ParseErrorKind::Unsupported(feature) => unsupported_message(*feature),
        ParseErrorKind::UnterminatedString => "文字列が閉じていません".to_string(),
        ParseErrorKind::InvalidEscape => {
            "不正なエスケープシーケンスです（`\\` を含む値は `'...'` の literal string で書けます）"
                .to_string()
        }
        ParseErrorKind::InvalidUnicodeEscape => {
            "`\\u` / `\\U` エスケープが Unicode スカラー値ではありません".to_string()
        }
        ParseErrorKind::ControlCharInString => "文字列に制御文字は書けません".to_string(),
        ParseErrorKind::ExpectedKey => "キーが必要です".to_string(),
        ParseErrorKind::ExpectedValue => "値が必要です".to_string(),
        ParseErrorKind::ExpectedEquals => "キーの後に `=` が必要です".to_string(),
        ParseErrorKind::ExpectedNewline => {
            "値の後に改行が必要です（1 行に書けるキーは 1 つ）".to_string()
        }
        ParseErrorKind::UnclosedArray => "配列が閉じていません（`]` がありません）".to_string(),
        ParseErrorKind::UnclosedTableHeader => {
            "テーブルヘッダが閉じていません（`]` がありません）".to_string()
        }
        ParseErrorKind::EmptyKey => "空のキーは書けません".to_string(),
        ParseErrorKind::IntegerOutOfRange => "整数が i64 の範囲を超えています".to_string(),
        ParseErrorKind::InvalidInteger => "整数リテラルが不正です".to_string(),
        ParseErrorKind::InvalidLiteral => {
            "値のリテラルが不正です（整数・小数・日時のどれとしても読めません）".to_string()
        }
        ParseErrorKind::DepthExceeded => "ネストが深すぎます（上限 128）".to_string(),
        _ => e.to_string(),
    }
}

/// kabosu v0.1 で未対応の構文。yuzu の設定で必要になる書き換え先を案内する
fn unsupported_message(feature: UnsupportedFeature) -> String {
    let (name, hint) = match feature {
        UnsupportedFeature::InlineTable => (
            "インラインテーブル（`{ ... }`）",
            "`[lint.terms]` のようなテーブルヘッダで書いてください",
        ),
        UnsupportedFeature::MultilineString => (
            "複数行文字列（`\"\"\"` / `'''`）",
            "1 行の文字列で書いてください",
        ),
        UnsupportedFeature::ArrayOfTables => (
            "テーブルの配列（`[[...]]`）",
            "yuzu の設定にテーブルの配列を取るキーはありません",
        ),
        UnsupportedFeature::Float => ("小数（float）", "yuzu の設定に小数を取るキーはありません"),
        UnsupportedFeature::DateTime => ("日付・時刻リテラル", "文字列として引用してください"),
        UnsupportedFeature::RadixInteger => ("16 / 8 / 2 進整数", "10 進で書いてください"),
        _ => ("この構文", "別の書き方にしてください"),
    };
    format!("{name}は {CONFIG_FILE_NAME} では使えません（kabosu v0.1 の未対応構文）。{hint}")
}

/// ディレクトリ設定の値がプロジェクトルート配下でない理由
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PathIssue {
    Absolute,
    EscapesRoot,
    RootItself,
}

impl PathIssue {
    fn reason(self) -> &'static str {
        match self {
            Self::Absolute => "絶対パスは指定できません",
            Self::EscapesRoot => ".. でプロジェクトルートの外へ出ることはできません",
            Self::RootItself => "プロジェクトルート自身は指定できません",
        }
    }
}

/// プロジェクトルート配下の相対ディレクトリ設定を検証して絶対パスへ解決する。
///
/// I/O を伴わない字句正規化なので、**まだ存在しないディレクトリでも判定できる**
/// （`yuzu_core::include` の `read_under_root` は canonicalize 方式で既存ファイル専用。
/// 実体に対する最終防御は `yuzu_core::output::remove_dir_all_under` が受け持つ）。
fn resolve_dir_setting(root: &Path, raw: &str) -> Result<PathBuf, PathIssue> {
    let mut parts = Vec::new();
    for c in Path::new(raw).components() {
        match c {
            // `.` は CurDir として現れるので、文字列比較では取りこぼす
            Component::CurDir => {}
            Component::Normal(s) => parts.push(s),
            Component::ParentDir => return Err(PathIssue::EscapesRoot),
            Component::RootDir | Component::Prefix(_) => return Err(PathIssue::Absolute),
        }
    }
    if parts.is_empty() {
        return Err(PathIssue::RootItself);
    }
    Ok(parts.iter().fold(root.to_path_buf(), |p, s| p.join(s)))
}

/// `.` と `..` を字句的に畳んだパスを返す（I/O なし）。
///
/// 保護対象の比較に使う。`root.join("a/../dist/content")` は文字列としては
/// `dist` の前方一致にならないため、正規化せずに比較すると
/// 「出力先が原稿を飲み込んでいる」を検出できない
fn lexically_normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            c => out.push(c.as_os_str()),
        }
    }
    out
}

/// パース済み `yuzu.toml` のキー（`["input", "dir"]` 形式）の 1 始まり (行, 列)。
/// キー不在なら None（呼び出し側が既定値へフォールバックする）
fn key_position(doc: &Document, path: &[&str]) -> Option<(usize, usize)> {
    let mut table = doc.root();
    let mut span = None;
    for (i, name) in path.iter().enumerate() {
        let entry = table.get(name)?;
        span = Some(entry.key_span());
        if i + 1 < path.len() {
            table = entry.node().as_table()?;
        }
    }
    Some(line_col(doc.source(), span?.start))
}

/// `yuzu.toml` に対する警告 1 件。
///
/// yuzu-config は yuzu-core に依存しない（凍結した依存グラフでは葉）ため、
/// `yuzu_core::Diagnostic` ではなく中立な値型で返し、cli 側で変換する
#[derive(Debug, Clone)]
pub struct ConfigDiagnostic {
    /// ルール ID（`config-path-outside-root`）
    pub rule: &'static str,
    /// キーのパス（`input.dir` 形式）
    pub key_path: String,
    /// 1 始まりの行
    pub line: usize,
    /// 1 始まりの列（バイト基準。診断の列規約に合わせる）
    pub col: usize,
    pub message: String,
}

/// この crate が発行する全ルール ID。yuzu-config は依存グラフの葉で
/// yuzu-core のレジストリ（`yuzu_core::rules`）を参照できないため、
/// ここに一覧を持ち、レジストリとの一致は yuzu-cli 側のテストが縛る。
/// 未知キー・重複キーは診断ではなく設定エラー（読み込み失敗）なのでここには無い
pub const CONFIG_RULES: &[&str] = &[RULE_PATH_OUTSIDE_ROOT];
const RULE_PATH_OUTSIDE_ROOT: &str = "config-path-outside-root";

/// バイトオフセットを 1 始まりの (行, 列) へ変換する（列はバイト基準）
fn line_col(text: &str, offset: usize) -> (usize, usize) {
    let head = &text[..offset.min(text.len())];
    let line = head.matches('\n').count() + 1;
    let col = head.rsplit_once('\n').map_or(head.len(), |(_, l)| l.len()) + 1;
    (line, col)
}

/// base_url を「常に先頭・末尾スラッシュ付き」の形へ正規化する。
/// フル URL（`https://…`）は末尾スラッシュのみ保証する。
/// CLI の `--base-url` 上書き（CI から configure-pages の base_path を渡す用途）でも使う
pub fn normalize_base_url(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return "/".to_string();
    }
    if trimmed.contains("://") {
        let mut s = trimmed.to_string();
        if !s.ends_with('/') {
            s.push('/');
        }
        return s;
    }
    let core = trimmed.trim_matches('/');
    if core.is_empty() {
        return "/".to_string();
    }
    format!("/{core}/")
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{normalize_base_url, parse_config};
    use crate::{Config, ConfigError};

    fn parse(text: &str) -> Result<Config, ConfigError> {
        parse_config(text, Path::new("yuzu.toml")).map(|(config, _)| config)
    }

    /// `Invalid` の (キーパス, 行, 列) の並び
    fn issues(text: &str) -> Vec<(String, usize, usize)> {
        match parse(text) {
            Err(ConfigError::Invalid { issues, .. }) => issues
                .into_iter()
                .map(|i| (i.key_path, i.line, i.col))
                .collect(),
            other => panic!("Invalid を期待: {other:?}"),
        }
    }

    #[test]
    fn 重複キーは構文エラーで先の定義の位置が付く() {
        let text = "[site]\ntitle = \"a\"\ntitle = \"b\"\n";
        match parse(text) {
            Err(ConfigError::Syntax {
                line, col, message, ..
            }) => {
                assert_eq!((line, col), (3, 1));
                assert!(message.contains("先の定義: 2:1"), "{message}");
            }
            other => panic!("Syntax を期待: {other:?}"),
        }
    }

    #[test]
    fn 問題がなければ読める() {
        let config = parse("[site]\ntitle = \"a\"\n").unwrap();
        assert_eq!(config.site.title, "a");
        assert!(parse("").is_ok(), "空ファイルは全キー既定");
    }

    #[test]
    fn 未知のトップレベルキーは対応キー一覧付きのエラーになる() {
        let text = "[markdwon]\ngfm = true\n";
        assert_eq!(issues(text), vec![("markdwon".to_string(), 1, 2)]);
        let Err(e) = parse(text) else { unreachable!() };
        let msg = e.to_string();
        assert!(msg.contains("未知のキー `markdwon`"), "{msg}");
        assert!(
            msg.contains("markdown"),
            "対応キーに正しい綴りが出る: {msg}"
        );
    }

    #[test]
    fn 入れ子の未知キーも検出する() {
        let text = "[markdown.crossreff]\nnumbering = \"site\"\n";
        assert_eq!(
            issues(text),
            vec![("markdown.crossreff".to_string(), 1, 11)]
        );
    }

    #[test]
    fn 未知キーの配下は検査しない() {
        // 親が未知なら子も当然未知だが、報告は親の 1 件だけにする
        let text = "[typo]\na = 1\n[typo.b]\nc = 2\n";
        assert_eq!(issues(text).len(), 1, "{:?}", issues(text));
    }

    #[test]
    fn 自由キーのマップは未知キー扱いしない() {
        // css_vars / css_vars_dark / lint.terms はユーザ任意の名前
        assert!(parse("[theme.css_vars]\n\"--accent\" = \"#0a6cff\"\n").is_ok());
        assert!(parse("[lint.terms]\n\"サーバ\" = [\"サーバー\"]\n").is_ok());
        // 用語集の辞書も同じ（登録し忘れるとユーザの略語が全部エラーになる）
        assert!(parse("[markdown.glossary.terms]\nSSG = \"静的サイト生成\"\n").is_ok());
        // 一方で glossary 自身のキーのタイポは拾う
        let typo = "[markdown.glossary]\npage_titel = \"用語集\"\n";
        assert_eq!(
            issues(typo),
            vec![("markdown.glossary.page_titel".to_string(), 2, 1)]
        );
    }

    #[test]
    fn 既知キーは値の型が合えば通る() {
        let text = "[markdown.mermaid]\nbackend = \"ssr\"\n[input]\nignore = [\"**/_drafts/**\"]\n[search]\nsynonyms = [[\"a\", \"b\"]]\n";
        let config = parse(text).unwrap();
        assert_eq!(config.markdown.mermaid.backend, crate::MermaidBackend::Ssr);
        assert_eq!(config.search.synonyms, vec![vec!["a", "b"]]);
    }

    #[test]
    fn 型不一致は日本語の文言で位置が付く() {
        let text = "[dev]\nport = \"5173\"\n";
        assert_eq!(issues(text), vec![("dev.port".to_string(), 2, 8)]);
        let Err(e) = parse(text) else { unreachable!() };
        let msg = e.to_string();
        assert!(msg.contains("期待: 整数、実際: 文字列"), "{msg}");
    }

    #[test]
    fn 整数の範囲外はエラー() {
        let text = "[dev]\nport = 70000\n";
        assert_eq!(issues(text).len(), 1);
        let Err(e) = parse(text) else { unreachable!() };
        assert!(e.to_string().contains("範囲外"), "{e}");
    }

    #[test]
    fn 列挙値の不正な値は選択肢付きのエラー() {
        let text = "[markdown.mermaid]\nbackend = \"server\"\n";
        assert_eq!(
            issues(text),
            vec![("markdown.mermaid.backend".to_string(), 2, 11)]
        );
        let Err(e) = parse(text) else { unreachable!() };
        let msg = e.to_string();
        assert!(msg.contains("`client` / `ssr`"), "{msg}");
    }

    #[test]
    fn 複数の問題は_1_回で全件報告される() {
        let text = "[site]\ntitel = \"a\"\n[dev]\nport = \"x\"\nopen = 1\n";
        let found = issues(text);
        assert_eq!(found.len(), 3, "{found:?}");
        // 位置順（主 span の開始位置）で並ぶ
        let lines: Vec<_> = found.iter().map(|(_, l, _)| *l).collect();
        assert_eq!(lines, vec![2, 4, 5]);
    }

    #[test]
    fn 未対応構文は書き換え先のヒント付きの構文エラー() {
        let text = "[lint]\nterms = { \"サーバ\" = [\"サーバー\"] }\n";
        match parse(text) {
            Err(ConfigError::Syntax { line, message, .. }) => {
                assert_eq!(line, 2);
                assert!(message.contains("インラインテーブル"), "{message}");
                assert!(message.contains("[lint.terms]"), "{message}");
            }
            other => panic!("Syntax を期待: {other:?}"),
        }
    }

    #[test]
    fn base_url_の正規化() {
        assert_eq!(normalize_base_url(""), "/");
        assert_eq!(normalize_base_url("/"), "/");
        assert_eq!(normalize_base_url("docs"), "/docs/");
        assert_eq!(normalize_base_url("/docs"), "/docs/");
        assert_eq!(normalize_base_url("docs/"), "/docs/");
        assert_eq!(normalize_base_url("/docs/"), "/docs/");
        assert_eq!(normalize_base_url("/a/b"), "/a/b/");
        assert_eq!(
            normalize_base_url("https://example.com/docs"),
            "https://example.com/docs/"
        );
    }
}
