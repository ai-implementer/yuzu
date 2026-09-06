//! 手書き decode（TableDecoder）の統合テスト。
//! 必須 / 任意 / 既定値 / ネスト / 未知キー 3 方針 / 診断の蓄積・ソート・上限を検証する。

use std::collections::BTreeMap;

use kabosu::{
    Datetime, DatetimeKind, Decode, DecodeContext, DecodeOptions, Diagnostic, DiagnosticCode, Node,
    Severity, TableDecoder, UnknownKeys,
};

#[derive(Debug, Default, PartialEq)]
struct Config {
    title: String,
    port: u16,
    tags: Vec<String>,
    description: Option<String>,
    vars: BTreeMap<String, String>,
    dev: DevConfig,
}

#[derive(Debug, Default, PartialEq)]
struct DevConfig {
    enabled: bool,
    host: String,
}

impl Decode for DevConfig {
    fn decode(node: &Node, cx: &mut DecodeContext<'_>) -> Option<Self> {
        let mut d = TableDecoder::new(node, cx)?;
        let enabled = d.optional("enabled");
        let host = d.optional("host");
        d.finish();
        Some(Self {
            enabled: enabled.unwrap_or(false),
            host: host.unwrap_or_else(|| String::from("127.0.0.1")),
        })
    }
}

impl Decode for Config {
    fn decode(node: &Node, cx: &mut DecodeContext<'_>) -> Option<Self> {
        let mut d = TableDecoder::new(node, cx)?;
        let title = d.required("title");
        let port = d.optional("port");
        let tags = d.optional("tags");
        let description = d.optional("description");
        let vars = d.optional("vars");
        let dev = d.optional("dev");
        d.finish();
        Some(Self {
            title: title.unwrap_or_default(),
            port: port.unwrap_or(5173),
            tags: tags.unwrap_or_default(),
            description,
            vars: vars.unwrap_or_default(),
            dev: dev.unwrap_or_default(),
        })
    }
}

#[test]
fn 必須_任意_既定値_ネストが揃って読める() {
    let report = kabosu::from_str::<Config>(
        "title = \"yuzu\"\ntags = [\"a\", \"b\"]\n[vars]\naccent = \"#333\"\n[dev]\nenabled = true\n",
    )
    .unwrap();
    assert!(!report.has_errors(), "{:?}", report.diagnostics());
    let config = report.value().unwrap();
    assert_eq!(config.title, "yuzu");
    assert_eq!(config.port, 5173, "省略は既定値");
    assert_eq!(config.tags, ["a", "b"]);
    assert_eq!(
        config.description, None,
        "TOML に null は無い = 省略が None"
    );
    assert_eq!(config.vars.get("accent").unwrap(), "#333");
    assert_eq!(
        config.dev,
        DevConfig {
            enabled: true,
            host: String::from("127.0.0.1")
        }
    );
}

#[test]
fn 必須キーの欠落はテーブル末尾の長さゼロ_span() {
    let src = "port = 80\n";
    let report = kabosu::from_str::<Config>(src).unwrap();
    assert!(report.has_errors());
    assert!(report.value().is_none(), "エラーがあれば値を返さない");
    let d = &report.diagnostics()[0];
    assert_eq!(*d.code(), DiagnosticCode::MissingKey);
    assert_eq!(d.span().start, d.span().end, "長さ 0");
    assert_eq!(d.span().start, src.len(), "ルートテーブルの末尾");
    assert_eq!(format!("{}", d.key_path()), "title");
}

#[test]
fn 型不一致でも兄弟キーの診断が続行される() {
    let report =
        kabosu::from_str::<Config>("title = 1\nport = \"x\"\ntags = [1, \"b\", 2]\n").unwrap();
    assert!(report.value().is_none());
    // title(型) + port(型) + tags[0](型) + tags[2](型) の 4 件が主 span 順で並ぶ
    let codes: Vec<_> = report
        .diagnostics()
        .iter()
        .map(|d| d.code().clone())
        .collect();
    assert_eq!(codes.len(), 4, "{codes:?}");
    let starts: Vec<_> = report
        .diagnostics()
        .iter()
        .map(|d| d.span().start)
        .collect();
    let mut sorted = starts.clone();
    sorted.sort();
    assert_eq!(starts, sorted, "主 span の開始位置で安定ソート");
    // 配列要素のキー経路は添字
    assert_eq!(format!("{}", report.diagnostics()[2].key_path()), "tags.0");
}

#[test]
fn 未知キーの_3_方針() {
    let src = "title = \"t\"\nzzz = 1\n";

    // Warn（既定）: 警告だけなら値を返す
    let report = kabosu::from_str::<Config>(src).unwrap();
    assert!(!report.has_errors());
    assert!(report.value().is_some());
    let d = &report.diagnostics()[0];
    // 対応キーは構造化して持つ（利用側が翻訳・候補提示に使う）
    assert_eq!(
        *d.code(),
        DiagnosticCode::UnknownKey {
            known_keys: ["title", "port", "tags", "description", "vars", "dev"]
                .map(String::from)
                .to_vec(),
        }
    );
    assert_eq!(d.severity(), Severity::Warning);
    assert!(d.message().contains("known keys:"), "{}", d.message());

    // Deny: エラーになり値を返さない
    let mut options = DecodeOptions::default();
    options.unknown_keys = UnknownKeys::Deny;
    let report = kabosu::from_str_with_options::<Config>(src, options).unwrap();
    assert!(report.has_errors());
    assert!(report.value().is_none());

    // Ignore: 診断なし
    let mut options = DecodeOptions::default();
    options.unknown_keys = UnknownKeys::Ignore;
    let report = kabosu::from_str_with_options::<Config>(src, options).unwrap();
    assert!(report.diagnostics().is_empty());
}

#[test]
fn 自由キーの_btreemap_は未知キー検査の対象外() {
    let report = kabosu::from_str::<Config>(
        "title = \"t\"\n[vars]\n\"--accent\" = \"#00f\"\nfoo = \"bar\"\n",
    )
    .unwrap();
    assert!(
        report.diagnostics().is_empty(),
        "{:?}",
        report.diagnostics()
    );
    assert_eq!(report.value().unwrap().vars.len(), 2);
}

#[test]
fn 診断は上限で打ち切られ省略件数が最後に付く() {
    // 上限 3 に対して型エラーを 5 個作る
    let src = "title = 1\nport = \"a\"\ntags = 2\ndescription = 3\nvars = 4\n";
    let mut options = DecodeOptions::default();
    options.max_diagnostics = 3;
    let report = kabosu::from_str_with_options::<Config>(src, options).unwrap();
    let diags = report.diagnostics();
    assert_eq!(diags.len(), 4, "3 件 + 省略通知");
    match diags.last().unwrap().code() {
        DiagnosticCode::TooManyDiagnostics { omitted } => assert_eq!(*omitted, 2),
        other => panic!("末尾は省略通知のはず: {other:?}"),
    }
}

#[test]
fn 整数の範囲検査と診断コード() {
    let report = kabosu::from_str::<Config>("title = \"t\"\nport = 70000\n").unwrap();
    assert!(report.value().is_none());
    let d = &report.diagnostics()[0];
    assert_eq!(*d.code(), DiagnosticCode::IntegerOutOfRange);
    assert!(d.message().contains("70000"), "{}", d.message());
}

#[test]
fn 独自診断を_raw_と_diagnostic_で追加できる() {
    struct Strict;
    impl Decode for Strict {
        fn decode(node: &Node, cx: &mut DecodeContext<'_>) -> Option<Self> {
            let mut d = TableDecoder::new(node, &mut *cx)?;
            let entry = d.raw("mode");
            d.finish();
            // raw で取ったエントリは finish 後も使える（テーブル借用は cx と独立）
            if let Some(entry) = entry {
                if entry.node().as_str() != Some("fast") {
                    let mut path = cx.key_path().clone();
                    path.push(kabosu::KeySegment::new(
                        String::from("mode"),
                        entry.key_span(),
                    ));
                    cx.diagnostic(Diagnostic::new(
                        DiagnosticCode::Custom("mode-invalid".into()),
                        Severity::Error,
                        String::from("mode must be \"fast\""),
                        path,
                        entry.node().span(),
                    ));
                    return None;
                }
            }
            Some(Strict)
        }
    }
    let report = kabosu::from_str::<Strict>("mode = \"slow\"\n").unwrap();
    assert!(report.has_errors());
    let d = &report.diagnostics()[0];
    assert_eq!(*d.code(), DiagnosticCode::Custom("mode-invalid".into()));
    // span から行列へ変換できる（DecodeContext::document 経由の使い方の確認）
    let doc = kabosu::Document::parse("mode = \"slow\"\n").unwrap();
    let lc = doc.line_col(d.span().start);
    assert_eq!((lc.line, lc.col), (1, 8));
}

#[test]
fn テーブルでない場所にテーブルを要求すると型不一致() {
    let report = kabosu::from_str::<Config>("title = \"t\"\ndev = 1\n").unwrap();
    assert!(report.value().is_none());
    let d = &report.diagnostics()[0];
    assert!(matches!(d.code(), DiagnosticCode::TypeMismatch { .. }));
    assert!(d.message().contains("expected table"), "{}", d.message());
}

#[test]
fn 上限で省略された_error_も_has_errors_に反映される() {
    // max_diagnostics = 0 だと必須キー欠落の Error は一覧から省略され、
    // TooManyDiagnostics の Warning だけが残る。それでも値は返さず has_errors は true
    let mut options = DecodeOptions::default();
    options.max_diagnostics = 0;
    let report = kabosu::from_str_with_options::<Config>("port = 1\n", options).unwrap();
    assert!(report.value().is_none(), "必須キー欠落なので値は無い");
    assert!(report.has_errors(), "省略された Error も数える");
    let diags = report.diagnostics();
    assert_eq!(diags.len(), 1, "{diags:?}");
    assert_eq!(
        *diags[0].code(),
        DiagnosticCode::TooManyDiagnostics { omitted: 1 }
    );
    assert_eq!(diags[0].severity(), Severity::Warning);
}

#[test]
fn float_は_f64_に_decode_できて整数リテラルは受けない() {
    let report =
        kabosu::from_str::<BTreeMap<String, f64>>("a = 1.5\nb = -inf\nc = 0x10\n").unwrap();
    assert!(
        report.value().is_none(),
        "0x10 は整数なので float 欄には入らない"
    );
    let d = &report.diagnostics()[0];
    assert!(
        matches!(
            d.code(),
            DiagnosticCode::TypeMismatch {
                expected: kabosu::ValueKind::Float,
                found: kabosu::ValueKind::Integer
            }
        ),
        "{:?}",
        d.code()
    );
    assert!(
        d.message().contains("expected float, found integer"),
        "{}",
        d.message()
    );

    let report = kabosu::from_str::<BTreeMap<String, f64>>("a = 1.5\nb = -inf\nn = nan\n").unwrap();
    let v = report.value().unwrap();
    assert_eq!(v["a"], 1.5);
    assert_eq!(v["b"], f64::NEG_INFINITY);
    assert!(v["n"].is_nan());

    // 逆方向: float を整数欄には入れない
    let report = kabosu::from_str::<BTreeMap<String, i64>>("a = 1.0\n").unwrap();
    assert!(report.value().is_none());
    assert!(
        report.diagnostics()[0]
            .message()
            .contains("expected integer, found float")
    );

    // 配列
    let report = kabosu::from_str::<BTreeMap<String, Vec<f64>>>("xs = [1.0, 2.5e3]\n").unwrap();
    assert_eq!(report.value().unwrap()["xs"], vec![1.0, 2500.0]);
}

#[test]
fn date_time_は_datetime_に_decode_できる() {
    let report = kabosu::from_str::<BTreeMap<String, Datetime>>(
        "odt = 1979-05-27T00:32:00.999999-07:00\n\
         ldt = 1979-05-27 07:32:00\n\
         ld = 1979-05-27\n\
         lt = 07:32:00.5\n",
    )
    .unwrap();
    assert!(!report.has_errors(), "{:?}", report.diagnostics());
    let v = report.value().unwrap();
    assert_eq!(v["odt"].kind(), DatetimeKind::OffsetDatetime);
    assert_eq!(v["odt"].offset().unwrap().minutes(), -420);
    assert_eq!(v["ldt"].kind(), DatetimeKind::LocalDatetime);
    assert_eq!(v["ld"].date().unwrap().day(), 27);
    assert_eq!(v["lt"].time().unwrap().nanosecond(), 500_000_000);

    // 型は厳格（文字列で書いた日付は日付欄に入らない）
    let report = kabosu::from_str::<BTreeMap<String, Datetime>>("a = \"1979-05-27\"\n").unwrap();
    assert!(report.value().is_none());
    assert!(
        report.diagnostics()[0]
            .message()
            .contains("expected datetime, found string"),
        "{}",
        report.diagnostics()[0].message()
    );

    // 逆方向: 日付を文字列欄には入れない
    let report = kabosu::from_str::<BTreeMap<String, String>>("a = 1979-05-27\n").unwrap();
    assert!(report.value().is_none());
    assert!(
        report.diagnostics()[0]
            .message()
            .contains("expected string, found datetime")
    );

    // 配列
    let report =
        kabosu::from_str::<BTreeMap<String, Vec<Datetime>>>("xs = [1979-05-27, 07:32:00]\n")
            .unwrap();
    let xs = &report.value().unwrap()["xs"];
    assert_eq!(xs[0].kind(), DatetimeKind::LocalDate);
    assert_eq!(xs[1].kind(), DatetimeKind::LocalTime);
}
