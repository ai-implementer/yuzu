//! OpenAPI 3.x・Swagger 2.0 / JSON Schema の Value 走査 → HTML 組み立て
//! （[`super::render_spec`] の本体）。2.0 のレイアウト差（definitions /
//! `in: body` / responses 直下の schema / produces・consumes）は
//! [`SpecVersion::V2`] の分岐でのみ吸収し、3.x パスの挙動は変えない。
//!
//! 設計:
//! - 入力（YAML / JSON）は `serde_yaml_ng` で `serde_json::Value` に読む。
//!   yuzu-render の serde_json は `preserve_order` 有効なので Map は記述順を保つ
//!   （= 出力は決定的。HashMap の非決定順序を混ぜない）
//! - 走査中の全テキストは [`escape_html`] を通してから埋め込む（XSS 安全）
//! - `$ref` は文書内（`#/...`）とプロジェクト内ファイル（`path#/pointer`）を解決。
//!   パス基準は「仕様ファイル内は参照元ファイル相対（OpenAPI ツールチェーン標準）、
//!   インラインブロック内はプロジェクトルート相対（`file:` と同じ規約）」。
//!   到達可能なファイルは描画前に全ロードし（[`DocSet`]、借用の自己参照を避ける
//!   二相方式）、訪問スタック `stack`（文書キー＋ポインタ）で循環をガードする。
//!   循環・未解決・リモート参照・読み込み失敗は本文の外に注記を出すに留める

use std::collections::HashMap;

use serde_json::Value;

use crate::highlight::escape_html as esc;

use super::{SpecFiles, SpecKind};

/// 入れ子スキーマ描画の深さ上限（これを超えたら「以降省略」）
const MAX_DEPTH: usize = 8;

/// 1 ブロックから到達できる参照ファイル数の上限（暴走・参照爆発のガード）
const MAX_FILES: usize = 16;

/// スキーマ合成キーワードと、そのセクション見出しラベル
const COMBINATORS: [(&str, &str); 3] = [
    ("oneOf", "いずれか（oneOf）"),
    ("anyOf", "いずれか（anyOf）"),
    ("allOf", "すべて（allOf）"),
];

/// 仕様テキストを HTML に変換する（[`super::render_spec`] から委譲される本体）
pub(super) fn render(
    kind: SpecKind,
    source: &str,
    origin: Option<&str>,
    files: &dyn SpecFiles,
) -> String {
    let value: Value = match serde_yaml_ng::from_str(source) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(error = %e, "API 仕様のパースに失敗しました");
            return super::error_box(&format!("パースに失敗しました: {e}"), source);
        }
    };

    let version = if kind == SpecKind::OpenApi {
        // `openapi: 3.x` → V3 / `swagger: "2.0"` → V2 / どちらでもなければ未対応。
        // YAML の裸の `swagger: 2.0`（Number）も plain_value が "2.0" に正規化する
        if value
            .get("openapi")
            .map(plain_value)
            .is_some_and(|v| v.starts_with('3'))
        {
            SpecVersion::V3
        } else if value.get("swagger").map(plain_value).as_deref() == Some("2.0") {
            SpecVersion::V2
        } else {
            let msg = "OpenAPI 3.x / Swagger 2.0 のみ対応しています\
                       （`openapi: 3.x.y` か `swagger: \"2.0\"` が必要です）";
            tracing::warn!("{msg}");
            return super::error_box(msg, source);
        }
    } else {
        // JSON Schema 単体に版の概念は無い（version は operation・一覧描画でしか使わない）
        SpecVersion::V3
    };

    // インラインブロックは空キー = プロジェクトルート基準。file: 参照は
    // そのパスがキーになり、文書内の相対 $ref の基準ディレクトリを与える
    let main_key = origin
        .map(|o| normalize_rel_path("", o).unwrap_or_else(|_| o.to_string()))
        .unwrap_or_default();
    let docs = load_documents(&main_key, value, files);
    let root = docs
        .docs
        .get(&main_key)
        .expect("load_documents はメイン文書を必ず登録する");
    let mut r = Renderer::new(&docs, main_key.clone(), version);

    match kind {
        SpecKind::OpenApi => r.render_openapi_document(root),
        SpecKind::JsonSchema => r.render_jsonschema_document(root),
    }
}

/// メイン文書と、そこから `$ref` で到達できる参照先ファイル群（描画前に全ロード済み）
struct DocSet {
    /// 正規化済みルート相対パス → パース済み文書（メイン文書は origin キーか空文字）
    docs: HashMap<String, Value>,
    /// 読み込み・パースに失敗したファイル → 理由（描画時に注記として出す）
    failed: HashMap<String, String>,
}

/// メイン文書から到達可能な参照ファイルをワークリストで全ロードする。
/// 既にキーがあるファイルは再ロードしない = ロード段の循環は構造的に起きない
fn load_documents(main_key: &str, main: Value, files: &dyn SpecFiles) -> DocSet {
    let mut docs = HashMap::new();
    let mut failed: HashMap<String, String> = HashMap::new();
    let mut pending: Vec<String> = Vec::new();

    collect_file_refs(main_key, &main, &mut pending);
    docs.insert(main_key.to_string(), main);

    while let Some(path) = pending.pop() {
        if docs.contains_key(&path) || failed.contains_key(&path) {
            continue;
        }
        // メイン文書を除いた参照ファイル数で上限を判定する
        if docs.len() > MAX_FILES {
            failed.insert(
                path,
                format!("参照ファイルが多すぎます（上限 {MAX_FILES}）"),
            );
            continue;
        }
        match files.read(&path) {
            Err(reason) => {
                tracing::warn!(file = %path, "仕様ファイルの読み込みに失敗: {reason}");
                failed.insert(path, reason);
            }
            Ok(text) => match serde_yaml_ng::from_str::<Value>(&text) {
                Err(e) => {
                    tracing::warn!(file = %path, error = %e, "仕様ファイルのパースに失敗");
                    failed.insert(path, format!("パースに失敗しました: {e}"));
                }
                Ok(v) => {
                    collect_file_refs(&path, &v, &mut pending);
                    docs.insert(path, v);
                }
            },
        }
    }
    DocSet { docs, failed }
}

/// 文書 `doc_key` 内の全 `$ref` からファイル参照を集める（正規化済みパスで push）。
/// 正規化に失敗する参照（ルート外等）は描画時に同じ分類が注記を出すのでここでは捨てる
fn collect_file_refs(doc_key: &str, v: &Value, out: &mut Vec<String>) {
    match v {
        Value::Object(map) => {
            if let Some(Value::String(r)) = map.get("$ref") {
                if let Some((path, _)) = split_file_ref(r) {
                    if let Ok(norm) = normalize_rel_path(&parent_dir(doc_key), path) {
                        out.push(norm);
                    }
                }
            }
            for val in map.values() {
                collect_file_refs(doc_key, val, out);
            }
        }
        Value::Array(arr) => {
            for val in arr {
                collect_file_refs(doc_key, val, out);
            }
        }
        _ => {}
    }
}

/// `$ref` の解決結果
enum Resolved<'a> {
    /// 解決成功（doc = 参照先の文書キー、pointer = 文書内 JSON ポインタ）
    Ok {
        doc: String,
        pointer: String,
        value: &'a Value,
    },
    /// 解決中の参照へ戻ってきた（循環）
    Cycle,
    /// リモート参照など対応しない形式
    Unsupported,
    /// ファイルの読み込み・パース・パス正規化の失敗（理由つき）
    Failed(String),
    /// 文書は読めたがポインタ先が存在しない
    Unresolved,
}

/// 描画対象の仕様バージョン（JSON Schema 単体は V3 扱いで固定。参照しない）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SpecVersion {
    /// Swagger 2.0（definitions / in:body / responses.schema / produces・consumes）
    V2,
    /// OpenAPI 3.x（components / requestBody / content）
    V3,
}

/// 走査状態。`docs` は `$ref` 解決の基点となる文書群、`cur_doc` は現在の文書キー、
/// `stack` は解決中の（文書キー, ポインタ）（循環ガード）
struct Renderer<'a> {
    docs: &'a DocSet,
    cur_doc: String,
    stack: Vec<(String, String)>,
    version: SpecVersion,
    /// Swagger 2.0 の top-level `consumes` / `produces`（operation 側が無いときの既定）
    v2_consumes: Option<&'a Vec<Value>>,
    v2_produces: Option<&'a Vec<Value>>,
}

impl<'a> Renderer<'a> {
    fn new(docs: &'a DocSet, main_key: String, version: SpecVersion) -> Self {
        Self {
            docs,
            cur_doc: main_key,
            stack: Vec::new(),
            version,
            v2_consumes: None,
            v2_produces: None,
        }
    }

    /// `cur_doc` を `doc` に切り替えて `f` を実行し、必ず元へ戻す
    fn with_doc<T>(&mut self, doc: String, f: impl FnOnce(&mut Self) -> T) -> T {
        let saved = std::mem::replace(&mut self.cur_doc, doc);
        let out = f(self);
        self.cur_doc = saved;
        out
    }

    /// `$ref` を現在の文書基準で分類・解決する
    fn resolve(&self, r: &str) -> Resolved<'a> {
        self.resolve_in(&self.cur_doc.clone(), r)
    }

    /// `$ref` を `base_doc` 基準で分類・解決する（描画はしない）。循環チェックまで行う
    fn resolve_in(&self, base_doc: &str, r: &str) -> Resolved<'a> {
        let (doc_key, pointer) = if let Some(ptr) = r.strip_prefix('#') {
            (base_doc.to_string(), ptr.to_string())
        } else if let Some((path, ptr)) = split_file_ref(r) {
            match normalize_rel_path(&parent_dir(base_doc), path) {
                Ok(norm) => (norm, ptr.to_string()),
                Err(reason) => return Resolved::Failed(reason),
            }
        } else {
            return Resolved::Unsupported;
        };
        if let Some(reason) = self.docs.failed.get(&doc_key) {
            return Resolved::Failed(reason.clone());
        }
        let Some(root) = self.docs.docs.get(&doc_key) else {
            // load_documents が到達可能な参照を全て処理するため通常来ない
            return Resolved::Failed("参照先を読み込めませんでした".to_string());
        };
        let value = if pointer.is_empty() {
            root
        } else {
            match root.pointer(&pointer) {
                Some(v) => v,
                None => return Resolved::Unresolved,
            }
        };
        if self
            .stack
            .iter()
            .any(|(d, p)| *d == doc_key && *p == pointer)
        {
            return Resolved::Cycle;
        }
        Resolved::Ok {
            doc: doc_key,
            pointer,
            value,
        }
    }

    // ---- 文書レベル ----

    fn render_openapi_document(&mut self, root: &'a Value) -> String {
        if self.version == SpecVersion::V2 {
            self.v2_consumes = root.get("consumes").and_then(|v| v.as_array());
            self.v2_produces = root.get("produces").and_then(|v| v.as_array());
        }
        let mut out = String::from("<section class=\"api-spec\">\n");
        out.push_str(&self.render_info(root.get("info")));
        if let Some(paths) = root.get("paths").and_then(|v| v.as_object()) {
            out.push_str("<div class=\"api-paths\">\n");
            for (path, item) in paths {
                out.push_str(&self.render_path_item(path, item));
            }
            out.push_str("</div>\n");
        }
        // 操作から参照されないスキーマも読めるよう、全スキーマの一覧を末尾に置く
        let (schemas, prefix) = match self.version {
            SpecVersion::V3 => (root.pointer("/components/schemas"), "/components/schemas"),
            SpecVersion::V2 => (root.get("definitions"), "/definitions"),
        };
        if let Some(map) = schemas.and_then(|v| v.as_object()) {
            if !map.is_empty() {
                out.push_str(&self.render_schema_index(map, prefix));
            }
        }
        out.push_str("</section>\n");
        out
    }

    /// components/schemas（2.0 は definitions）の全スキーマ一覧。
    /// 各エントリは閉じた details で、スキーマ名を summary に出す（記述順 = 決定的）
    fn render_schema_index(
        &mut self,
        map: &'a serde_json::Map<String, Value>,
        prefix: &str,
    ) -> String {
        let mut out = String::from(
            "<div class=\"api-schemas\"><p class=\"api-section-label\">スキーマ</p>\n",
        );
        for (name, schema) in map {
            out.push_str(&format!(
                "<details class=\"api-schema-def\"><summary>{}</summary>\n",
                code(name)
            ));
            // 自己参照スキーマ（Node → Node）を $ref 側の循環ガードで検出できるよう、
            // このスキーマ自身のポインタを解決スタックに積んでから描画する
            self.stack.push((
                self.cur_doc.clone(),
                format!("{prefix}/{}", pointer_escape(name)),
            ));
            // スカラ型は render_scalar が description を自前で出す（二重表示回避）
            if !is_scalar_like(schema) {
                if let Some(d) = schema.get("description").and_then(|v| v.as_str()) {
                    out.push_str(&format!("<p class=\"api-desc\">{}</p>\n", esc(d)));
                }
            }
            out.push_str(&self.render_schema(schema, 0));
            self.stack.pop();
            out.push_str("</details>\n");
        }
        out.push_str("</div>\n");
        out
    }

    fn render_jsonschema_document(&mut self, root: &'a Value) -> String {
        let mut out = String::from("<section class=\"api-spec api-schema\">\n");
        if let Some(title) = root.get("title").and_then(|v| v.as_str()) {
            out.push_str(&format!(
                "<div class=\"api-title\"><strong>{}</strong></div>\n",
                esc(title)
            ));
        }
        // スカラ型ルートは render_scalar が description を自前で出すため、
        // ここでも出すと二重表示になる（object/array/combinator/$ref は出さないので担う）
        if !is_scalar_like(root) {
            if let Some(d) = root.get("description").and_then(|v| v.as_str()) {
                out.push_str(&format!("<p class=\"api-desc\">{}</p>\n", esc(d)));
            }
        }
        out.push_str(&self.render_schema(root, 0));
        out.push_str("</section>\n");
        out
    }

    fn render_info(&self, info: Option<&Value>) -> String {
        let mut out = String::from("<header class=\"api-info\">\n");
        if let Some(info) = info {
            if let Some(title) = info.get("title").and_then(|v| v.as_str()) {
                // 見出し階層（h1〜h6）を汚さないよう div/strong で表現する
                out.push_str(&format!(
                    "<div class=\"api-title\"><strong>{}</strong>",
                    esc(title)
                ));
                if let Some(ver) = info.get("version") {
                    out.push_str(&format!(
                        " <span class=\"api-version\">v{}</span>",
                        esc(&plain_value(ver))
                    ));
                }
                out.push_str("</div>\n");
            }
            if let Some(d) = info.get("description").and_then(|v| v.as_str()) {
                out.push_str(&format!("<p class=\"api-desc\">{}</p>\n", esc(d)));
            }
        }
        out.push_str("</header>\n");
        out
    }

    // ---- パス / オペレーション ----

    fn render_path_item(&mut self, path: &str, item: &'a Value) -> String {
        let (item_doc, item) = self.deref(item);
        let path_params = item.get("parameters").and_then(|v| v.as_array());
        let mut out = String::new();
        for method in [
            "get", "put", "post", "delete", "patch", "head", "options", "trace",
        ] {
            if let Some(op) = item.get(method) {
                out.push_str(&self.render_operation(path, method, op, &item_doc, path_params));
            }
        }
        out
    }

    fn render_operation(
        &mut self,
        path: &str,
        method: &str,
        op: &'a Value,
        item_doc: &str,
        path_params: Option<&'a Vec<Value>>,
    ) -> String {
        let (op_doc, op) = self.deref_in(item_doc, op);
        let upper = method.to_uppercase();
        let mut out = format!(
            "<details class=\"api-op api-op-{method}\">\n<summary><span class=\"api-method api-method-{method}\">{upper}</span> <code>{path}</code>",
            path = esc(path)
        );
        if let Some(s) = op.get("summary").and_then(|v| v.as_str()) {
            out.push_str(&format!(" <span class=\"api-summary\">{}</span>", esc(s)));
        }
        out.push_str("</summary>\n");

        if let Some(d) = op.get("description").and_then(|v| v.as_str()) {
            out.push_str(&format!("<p class=\"api-op-desc\">{}</p>\n", esc(d)));
        }

        let op_params = op.get("parameters").and_then(|v| v.as_array());
        let merged = self.merge_parameters(item_doc, path_params, &op_doc, op_params);
        // Swagger 2.0 はリクエストボディを `in: body` パラメータで表す。
        // パラメータ表からは分離してボディとして描画する（仕様上 body は最大 1 個）
        let (body_params, merged): (Vec<_>, Vec<_>) = if self.version == SpecVersion::V2 {
            merged
                .into_iter()
                .partition(|(_, p)| p.get("in").and_then(|v| v.as_str()) == Some("body"))
        } else {
            (Vec::new(), merged)
        };
        if !merged.is_empty() {
            out.push_str(&self.render_parameters(&merged));
        }
        match self.version {
            SpecVersion::V3 => self.with_doc(op_doc, |s| {
                if let Some(rb) = op.get("requestBody") {
                    out.push_str(&s.render_request_body(rb, 0));
                }
                if let Some(resp) = op.get("responses") {
                    out.push_str(&s.render_responses(resp, 0));
                }
            }),
            SpecVersion::V2 => {
                if let Some((param_doc, param)) = body_params.into_iter().next() {
                    out.push_str(&self.render_v2_request_body(param_doc, param, op));
                }
                if let Some(resp) = op.get("responses") {
                    let inner = self.with_doc(op_doc, |s| s.render_v2_responses(resp, op));
                    out.push_str(&inner);
                }
            }
        }
        out.push_str("</details>\n");
        out
    }

    /// Swagger 2.0 の `in: body` パラメータをリクエストボディとして描画する
    /// （3.x の requestBody と同じ見た目。メディアタイプは consumes から）
    fn render_v2_request_body(
        &mut self,
        param_doc: String,
        param: &'a Value,
        op: &'a Value,
    ) -> String {
        let mut out = String::from(
            "<div class=\"api-request-body\"><p class=\"api-section-label\">リクエストボディ</p>\n",
        );
        if param.get("required").and_then(|v| v.as_bool()) == Some(true) {
            out.push_str("<p class=\"api-required-note\">必須</p>\n");
        }
        if let Some(d) = param.get("description").and_then(|v| v.as_str()) {
            out.push_str(&format!("<p class=\"api-desc\">{}</p>\n", esc(d)));
        }
        out.push_str(&self.v2_media_line(op, "consumes"));
        if let Some(schema) = param.get("schema") {
            let inner = self.with_doc(param_doc, |s| s.render_schema(schema, 0));
            out.push_str(&inner);
        }
        out.push_str("</div>\n");
        out
    }

    /// Swagger 2.0 のレスポンス群（response 直下に `schema` を持つ。3.x の content は無い）
    fn render_v2_responses(&mut self, resp: &'a Value, op: &'a Value) -> String {
        let Some(obj) = resp.as_object() else {
            return String::new();
        };
        let mut out = String::from(
            "<div class=\"api-responses\"><p class=\"api-section-label\">レスポンス</p>\n",
        );
        for (code_str, r) in obj {
            let (doc, r) = self.deref(r);
            let cls = status_class(code_str);
            let desc = r.get("description").and_then(|v| v.as_str()).unwrap_or("");
            out.push_str(&format!(
                "<div class=\"api-response\">\
                 <span class=\"api-status api-status-{cls}\">{code_e}</span> \
                 <span class=\"api-status-desc\">{desc_e}</span></div>\n",
                code_e = esc(code_str),
                desc_e = esc(desc),
            ));
            if let Some(schema) = r.get("schema") {
                out.push_str(&self.v2_media_line(op, "produces"));
                let inner = self.with_doc(doc, |s| s.render_schema(schema, 0));
                out.push_str(&inner);
            }
        }
        out.push_str("</div>\n");
        out
    }

    /// Swagger 2.0 の consumes / produces のメディアタイプ行。operation 側があれば
    /// 優先し、無ければ top-level の既定を使う。どちらも無ければ空文字（行を出さない）。
    /// 2.0 はメディアタイプごとにスキーマが分かれないため 1 行に列挙する（記述順のまま）
    fn v2_media_line(&self, op: &'a Value, key: &str) -> String {
        let list = op.get(key).and_then(|v| v.as_array()).or(match key {
            "consumes" => self.v2_consumes,
            _ => self.v2_produces,
        });
        let Some(list) = list else {
            return String::new();
        };
        let media: Vec<String> = list
            .iter()
            .map(|v| format!("<code>{}</code>", esc(&plain_value(v))))
            .collect();
        if media.is_empty() {
            return String::new();
        }
        format!("<div class=\"api-media\">{}</div>\n", media.join(", "))
    }

    /// path-item レベルと operation レベルの parameters をマージする。
    /// 同名・同 in の重複は operation を優先し、path-item 側を落とす。
    /// 各要素はファイル跨ぎ deref 済みで、実体が属する文書キーを伴う
    fn merge_parameters(
        &self,
        item_doc: &str,
        path_params: Option<&'a Vec<Value>>,
        op_doc: &str,
        op_params: Option<&'a Vec<Value>>,
    ) -> Vec<(String, &'a Value)> {
        let op: Vec<(String, &Value)> = op_params
            .map(|a| a.iter().map(|p| self.deref_in(op_doc, p)).collect())
            .unwrap_or_default();
        let op_keys: Vec<(String, String)> = op.iter().map(|(_, p)| param_key(p)).collect();

        let mut result: Vec<(String, &Value)> = Vec::new();
        if let Some(pp) = path_params {
            for p in pp {
                let (doc, p) = self.deref_in(item_doc, p);
                let key = param_key(p);
                if op_keys.contains(&key) {
                    continue; // operation 側で上書きされる
                }
                result.push((doc, p));
            }
        }
        result.extend(op);
        result
    }

    fn render_parameters(&mut self, params: &[(String, &'a Value)]) -> String {
        let mut out = String::from(
            "<div class=\"api-params\"><p class=\"api-section-label\">パラメータ</p>\n\
             <table class=\"api-schema-table\">\n\
             <thead><tr><th>名前</th><th>場所</th><th>型</th><th>必須</th><th>説明</th></tr></thead>\n\
             <tbody>\n",
        );
        for (doc, p) in params {
            let name = p.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let loc = p.get("in").and_then(|v| v.as_str()).unwrap_or("");
            let schema = p.get("schema");
            // Swagger 2.0 の非 body パラメータは型情報（type/format/items/enum）を
            // パラメータ自身が直接持つため、schema が無ければ自身を型ソースにする。
            // 3.x（content 持ち等で schema 無し）は従来どおり「—」表示のまま
            let schema = match (schema, self.version) {
                (None, SpecVersion::V2) => Some(*p),
                (s, _) => s,
            };
            // 型ラベル・注記内の $ref はパラメータ実体が属する文書を基準に解決する
            let (ty, annos) = self.with_doc(doc.clone(), |s| {
                (
                    schema.map_or_else(|| "—".to_string(), |sc| s.type_label(sc)),
                    schema.map(|sc| s.annotations_html(sc)).unwrap_or_default(),
                )
            });
            // path パラメータは仕様上つねに必須
            let required =
                p.get("required").and_then(|v| v.as_bool()) == Some(true) || loc == "path";
            let req_mark = if required { "✓" } else { "" };

            let mut desc = String::new();
            if let Some(d) = p.get("description").and_then(|v| v.as_str()) {
                desc.push_str(&esc(d));
            }
            desc.push_str(&annos);

            out.push_str(&format!(
                "<tr><td>{name_c}</td><td><code>{loc_e}</code></td><td>{ty_c}</td>\
                 <td class=\"api-req\">{req_mark}</td><td>{desc}</td></tr>\n",
                name_c = code(name),
                loc_e = esc(loc),
                ty_c = code(&ty),
            ));
        }
        out.push_str("</tbody></table></div>\n");
        out
    }

    fn render_request_body(&mut self, rb: &'a Value, depth: usize) -> String {
        let (doc, rb) = self.deref(rb);
        let mut out = String::from(
            "<div class=\"api-request-body\"><p class=\"api-section-label\">リクエストボディ</p>\n",
        );
        if rb.get("required").and_then(|v| v.as_bool()) == Some(true) {
            out.push_str("<p class=\"api-required-note\">必須</p>\n");
        }
        if let Some(d) = rb.get("description").and_then(|v| v.as_str()) {
            out.push_str(&format!("<p class=\"api-desc\">{}</p>\n", esc(d)));
        }
        if let Some(content) = rb.get("content") {
            let inner = self.with_doc(doc, |s| s.render_content(content, depth));
            out.push_str(&inner);
        }
        out.push_str("</div>\n");
        out
    }

    fn render_responses(&mut self, resp: &'a Value, depth: usize) -> String {
        let Some(obj) = resp.as_object() else {
            return String::new();
        };
        let mut out = String::from(
            "<div class=\"api-responses\"><p class=\"api-section-label\">レスポンス</p>\n",
        );
        for (code_str, r) in obj {
            let (doc, r) = self.deref(r);
            let cls = status_class(code_str);
            let desc = r.get("description").and_then(|v| v.as_str()).unwrap_or("");
            out.push_str(&format!(
                "<div class=\"api-response\">\
                 <span class=\"api-status api-status-{cls}\">{code_e}</span> \
                 <span class=\"api-status-desc\">{desc_e}</span></div>\n",
                code_e = esc(code_str),
                desc_e = esc(desc),
            ));
            if let Some(content) = r.get("content") {
                let inner = self.with_doc(doc, |s| s.render_content(content, depth));
                out.push_str(&inner);
            }
        }
        out.push_str("</div>\n");
        out
    }

    /// content（メディアタイプ→スキーマ）を描画。application/json を先頭に、残りは記述順
    fn render_content(&mut self, content: &'a Value, depth: usize) -> String {
        let Some(obj) = content.as_object() else {
            return String::new();
        };
        let mut entries: Vec<(&String, &'a Value)> = obj.iter().collect();
        // 安定ソートなので application/json 以外の相対順は記述順のまま保たれる
        entries.sort_by_key(|(k, _)| u8::from(k.as_str() != "application/json"));

        let mut out = String::new();
        for (media, mt) in entries {
            out.push_str(&format!(
                "<div class=\"api-media\"><code>{}</code></div>\n",
                esc(media)
            ));
            if let Some(schema) = mt.get("schema") {
                out.push_str(&self.render_schema(schema, depth));
            }
        }
        out
    }

    // ---- スキーマ ----

    /// スキーマ 1 個を「構造ブロック」として描画する（object → テーブル、array → 要素、
    /// combinator → 列挙、scalar → 型注記）。`depth` は入れ子の深さ
    fn render_schema(&mut self, schema: &'a Value, depth: usize) -> String {
        if depth > MAX_DEPTH {
            return "<p class=\"api-omitted\">（ネストが深いため以降省略）</p>\n".to_string();
        }
        if let Some(r) = ref_str(schema) {
            return self.render_ref(r, depth);
        }
        for (key, label) in COMBINATORS {
            if let Some(arr) = schema.get(key).and_then(|v| v.as_array()) {
                return self.render_combinator(label, arr, depth);
            }
        }

        let ty = schema.get("type").and_then(|v| v.as_str());
        if ty == Some("object") || (ty.is_none() && schema.get("properties").is_some()) {
            return self.render_object(schema, depth);
        }
        if ty == Some("array") || (ty.is_none() && schema.get("items").is_some()) {
            return self.render_array(schema, depth);
        }
        self.render_scalar(schema)
    }

    /// `$ref` を解決して描画する。ファイル跨ぎは cur_doc を切り替えて再帰し、
    /// 循環は（文書キー, ポインタ）のスタックで止める
    fn render_ref(&mut self, r: &str, depth: usize) -> String {
        match self.resolve(r) {
            Resolved::Ok {
                doc,
                pointer,
                value,
            } => {
                self.stack.push((doc.clone(), pointer));
                let body = self.with_doc(doc, |s| s.render_schema(value, depth));
                self.stack.pop();
                // どのスキーマを展開したかを見出しに示す
                format!(
                    "<p class=\"api-ref-name\">{}</p>\n{body}",
                    code(ref_name(r))
                )
            }
            Resolved::Cycle => format!(
                "<p class=\"api-ref\">{}（循環参照）</p>\n",
                code(ref_name(r))
            ),
            Resolved::Unsupported => format!(
                "<p class=\"api-ref\">{}（未対応の参照）</p>\n",
                code(ref_name(r))
            ),
            Resolved::Failed(reason) => format!(
                "<p class=\"api-ref\">{}（読み込み失敗: {}）</p>\n",
                code(ref_name(r)),
                esc(&reason)
            ),
            Resolved::Unresolved => format!(
                "<p class=\"api-ref\">{}（未解決の参照）</p>\n",
                code(ref_name(r))
            ),
        }
    }

    fn render_object(&mut self, schema: &'a Value, depth: usize) -> String {
        let required = required_names(schema);
        let props = schema.get("properties").and_then(|v| v.as_object());

        let mut out = String::from(
            "<table class=\"api-schema-table\">\n\
             <thead><tr><th>プロパティ</th><th>型</th><th>必須</th><th>説明</th></tr></thead>\n\
             <tbody>\n",
        );
        match props {
            Some(props) if !props.is_empty() => {
                for (name, ps) in props {
                    let ty = self.type_label(ps);
                    let req = required.contains(&name.as_str());
                    let req_mark = if req { "✓" } else { "" };
                    let desc = self.desc_cell(ps, ps);
                    out.push_str(&format!(
                        "<tr><td>{name_c}</td><td>{ty_c}</td>\
                         <td class=\"api-req\">{req_mark}</td><td>{desc}</td></tr>\n",
                        name_c = code(name),
                        ty_c = code(&ty),
                    ));
                    if self.should_expand(ps) {
                        let inner = self.render_schema(ps, depth + 1);
                        if !inner.trim().is_empty() {
                            out.push_str(&format!(
                                "<tr class=\"api-nested-row\"><td colspan=\"4\">\
                                 <details class=\"api-nested\"><summary>{} の詳細</summary>\n\
                                 {inner}</details></td></tr>\n",
                                code(name),
                            ));
                        }
                    }
                }
            }
            _ => {
                out.push_str(
                    "<tr><td colspan=\"4\" class=\"api-empty\">\
                     プロパティ定義なし（自由形式オブジェクト）</td></tr>\n",
                );
            }
        }
        out.push_str("</tbody></table>\n");
        out
    }

    fn render_array(&mut self, schema: &'a Value, depth: usize) -> String {
        let mut out = String::new();
        if let Some(items) = schema.get("items") {
            let ty = self.type_label(items);
            out.push_str(&format!(
                "<p class=\"api-array-items\">要素の型: {}</p>\n",
                code(&ty)
            ));
            if self.should_expand(items) {
                out.push_str(&self.render_schema(items, depth + 1));
            }
        } else {
            out.push_str("<p class=\"api-array-items\">要素の型は未指定です</p>\n");
        }
        out
    }

    fn render_combinator(&mut self, label: &str, subs: &'a [Value], depth: usize) -> String {
        let mut out = format!(
            "<div class=\"api-combinator\"><p class=\"api-combinator-label\">{}</p>\n\
             <ol class=\"api-combinator-list\">\n",
            esc(label)
        );
        for sub in subs {
            let ty = self.type_label(sub);
            out.push_str(&format!(
                "<li><span class=\"api-type\">{}</span>",
                code(&ty)
            ));
            let desc = self.desc_cell(sub, sub);
            if !desc.is_empty() {
                out.push_str(&format!(" {desc}"));
            }
            if self.should_expand(sub) {
                out.push('\n');
                out.push_str(&self.render_schema(sub, depth + 1));
            }
            out.push_str("</li>\n");
        }
        out.push_str("</ol></div>\n");
        out
    }

    fn render_scalar(&self, schema: &Value) -> String {
        let mut out = String::new();
        if let Some(d) = schema.get("description").and_then(|v| v.as_str()) {
            out.push_str(&format!("<p class=\"api-desc\">{}</p>\n", esc(d)));
        }
        let ty = self.type_label(schema);
        out.push_str(&format!(
            "<p class=\"api-scalar\">型: {}{}</p>\n",
            code(&ty),
            self.annotations_html(schema)
        ));
        out
    }

    /// プロパティ行を入れ子で展開すべきか（object / 複合型を持つ array / combinator /
    /// 解決可能で循環しない `$ref` のみ true）
    fn should_expand(&self, schema: &Value) -> bool {
        if let Some(r) = ref_str(schema) {
            return matches!(self.resolve(r), Resolved::Ok { .. });
        }
        if COMBINATORS
            .iter()
            .any(|(k, _)| schema.get(k).and_then(|v| v.as_array()).is_some())
        {
            return true;
        }
        let ty = schema.get("type").and_then(|v| v.as_str());
        if ty == Some("object") || (ty.is_none() && schema.get("properties").is_some()) {
            return schema
                .get("properties")
                .and_then(|v| v.as_object())
                .is_some_and(|o| !o.is_empty());
        }
        if ty == Some("array") || (ty.is_none() && schema.get("items").is_some()) {
            if let Some(items) = schema.get("items") {
                return self.should_expand(items);
            }
        }
        false
    }

    /// 「型」列に出す短い型ラベル（プレーンテキスト。埋め込み側でエスケープする）
    fn type_label(&self, schema: &Value) -> String {
        if let Some(r) = ref_str(schema) {
            let name = ref_name(r);
            return match self.resolve(r) {
                Resolved::Ok { .. } => name.to_string(),
                Resolved::Cycle => format!("{name}（循環参照）"),
                Resolved::Unsupported => format!("{name}（未対応の参照）"),
                Resolved::Failed(_) => format!("{name}（読み込み失敗）"),
                Resolved::Unresolved => format!("{name}（未解決の参照）"),
            };
        }
        for key in ["oneOf", "anyOf", "allOf"] {
            if schema.get(key).is_some() {
                return key.to_string();
            }
        }
        match schema.get("type") {
            Some(Value::String(s)) if s == "array" => {
                let inner = schema
                    .get("items")
                    .map_or_else(|| "any".to_string(), |i| self.type_label(i));
                format!("{inner}[]")
            }
            Some(Value::String(s)) => match schema.get("format").and_then(|v| v.as_str()) {
                Some(fmt) => format!("{s} ({fmt})"),
                None => s.clone(),
            },
            Some(Value::Array(arr)) => {
                let types: Vec<&str> = arr
                    .iter()
                    .filter_map(|v| v.as_str())
                    .filter(|s| *s != "null")
                    .collect();
                if types.is_empty() {
                    "any".to_string()
                } else {
                    types.join(" | ")
                }
            }
            _ => {
                if schema.get("properties").is_some() {
                    "object".to_string()
                } else if let Some(items) = schema.get("items") {
                    format!("{}[]", self.type_label(items))
                } else if schema.get("enum").is_some() {
                    "enum".to_string()
                } else {
                    "any".to_string()
                }
            }
        }
    }

    /// 説明セル: `desc_source` の description に、`schema` 由来の注記を併記する。
    /// パラメータのように description と型の持ち主が別なケースがあるので分けて受ける
    fn desc_cell(&self, desc_source: &Value, schema: &Value) -> String {
        let mut out = String::new();
        if let Some(d) = desc_source.get("description").and_then(|v| v.as_str()) {
            out.push_str(&esc(d));
        }
        out.push_str(&self.annotations_html(schema));
        out
    }

    /// nullable / enum / default / example を小さな注記チップで併記する
    fn annotations_html(&self, schema: &Value) -> String {
        let mut out = String::new();
        if nullable(schema) {
            out.push_str(" <span class=\"api-anno\">null 許容</span>");
        }
        if let Some(en) = schema.get("enum").and_then(|v| v.as_array()) {
            let joined = en
                .iter()
                .map(|v| code(&plain_value(v)))
                .collect::<Vec<_>>()
                .join(" / ");
            out.push_str(&format!(" <span class=\"api-anno\">値: {joined}</span>"));
        }
        if let Some(def) = schema.get("default") {
            out.push_str(&format!(
                " <span class=\"api-anno\">既定: {}</span>",
                code(&plain_value(def))
            ));
        }
        if let Some(ex) = schema.get("example") {
            out.push_str(&format!(
                " <span class=\"api-anno\">例: {}</span>",
                code(&plain_value(ex))
            ));
        }
        out
    }

    /// パラメータ / requestBody / response ラッパの `$ref` を実体まで剥がす
    /// （現在の文書基準）。実体が属する文書キーも返す
    fn deref(&self, v: &'a Value) -> (String, &'a Value) {
        self.deref_in(&self.cur_doc.clone(), v)
    }

    /// パラメータ / requestBody / response ラッパの `$ref` を `base_doc` 基準で
    /// 実体まで剥がす。ファイル跨ぎに対応するため、実体が属する文書キーも返す
    /// （呼び出し側が with_doc で cur_doc を切り替えてから中身を使う）。
    /// 解決できない参照はそこで止めてラッパをそのまま返す（下流は素通しで描画）
    fn deref_in(&self, base_doc: &str, v: &'a Value) -> (String, &'a Value) {
        let mut doc = base_doc.to_string();
        let mut cur = v;
        for _ in 0..MAX_DEPTH {
            let Some(r) = ref_str(cur) else { break };
            match self.resolve_in(&doc, r) {
                Resolved::Ok {
                    doc: next_doc,
                    value,
                    ..
                } => {
                    doc = next_doc;
                    cur = value;
                }
                _ => break,
            }
        }
        (doc, cur)
    }
}

// ---- 自由関数 ----

/// `<code>…</code>` で包む（内側はエスケープ）
fn code(text: &str) -> String {
    format!("<code>{}</code>", esc(text))
}

/// `$ref` の文字列を取り出す
fn ref_str(schema: &Value) -> Option<&str> {
    schema.get("$ref").and_then(|v| v.as_str())
}

/// render_schema がスカラ分岐（render_scalar）に落とすスキーマか。
/// render_scalar は description を自前で出すため、jsonschema 文書ヘッダ側の
/// description 出力と二重にならないよう出し分けに使う。
/// **render_schema の dispatch と同期を保つこと**（$ref / combinator /
/// object / array のいずれでもない → スカラ）
fn is_scalar_like(schema: &Value) -> bool {
    if ref_str(schema).is_some() {
        return false;
    }
    if COMBINATORS
        .iter()
        .any(|(k, _)| schema.get(k).and_then(|v| v.as_array()).is_some())
    {
        return false;
    }
    let ty = schema.get("type").and_then(|v| v.as_str());
    if ty == Some("object") || (ty.is_none() && schema.get("properties").is_some()) {
        return false;
    }
    if ty == Some("array") || (ty.is_none() && schema.get("items").is_some()) {
        return false;
    }
    true
}

/// ファイル参照（`path#/pointer` または `path`）ならパスとポインタに分割する。
/// ローカル参照（`#...`）とリモート参照（`scheme://`）は None
fn split_file_ref(r: &str) -> Option<(&str, &str)> {
    if r.starts_with('#') || r.contains("://") || r.is_empty() {
        return None;
    }
    match r.split_once('#') {
        Some((path, ptr)) => Some((path, ptr)),
        None => Some((r, "")), // fragment なし = 文書全体を参照
    }
}

/// 文書キー（ルート相対パス）の親ディレクトリ。インライン（空文字）はルート
fn parent_dir(doc_key: &str) -> String {
    match doc_key.rsplit_once('/') {
        Some((dir, _)) => dir.to_string(),
        None => String::new(),
    }
}

/// `base_dir` 基準の相対パスを `.` / `..` を字句処理してルート相対の正規形にする。
/// ルートを突き抜けたら Err（loader 側の canonicalize 検査と二重防御）
fn normalize_rel_path(base_dir: &str, rel: &str) -> Result<String, String> {
    let mut parts: Vec<&str> = if base_dir.is_empty() {
        Vec::new()
    } else {
        base_dir.split('/').collect()
    };
    for seg in rel.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                if parts.pop().is_none() {
                    return Err(format!("参照 {rel} はプロジェクトルートの外を指しています"));
                }
            }
            s => parts.push(s),
        }
    }
    if parts.is_empty() {
        return Err(format!("参照 {rel} のパスが空です"));
    }
    Ok(parts.join("/"))
}

/// 参照の表示名（末尾セグメント）。`#/components/schemas/User` → `User`
fn ref_name(r: &str) -> &str {
    r.rsplit('/').next().filter(|s| !s.is_empty()).unwrap_or(r)
}

/// object の required 配列（文字列のみ）
fn required_names(schema: &Value) -> Vec<&str> {
    schema
        .get("required")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default()
}

/// nullable 判定（OpenAPI 3.0 の `nullable: true` と 3.1 の `type: [..., "null"]` 両対応）
fn nullable(schema: &Value) -> bool {
    if schema.get("nullable").and_then(|v| v.as_bool()) == Some(true) {
        return true;
    }
    schema
        .get("type")
        .and_then(|v| v.as_array())
        .is_some_and(|a| a.iter().any(|v| v.as_str() == Some("null")))
}

/// パラメータの一意キー `(name, in)`
fn param_key(p: &Value) -> (String, String) {
    let name = p.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let loc = p.get("in").and_then(|v| v.as_str()).unwrap_or("");
    (name.to_string(), loc.to_string())
}

/// ステータスコードの区分クラス（`2xx` 等）
fn status_class(code: &str) -> &'static str {
    match code.chars().next() {
        Some('1') => "1xx",
        Some('2') => "2xx",
        Some('3') => "3xx",
        Some('4') => "4xx",
        Some('5') => "5xx",
        _ => "default",
    }
}

/// JSON Pointer のトークンエスケープ（RFC 6901: `~` → `~0`、`/` → `~1`）。
/// スキーマ一覧の循環ガードで $ref 側のポインタ表現と一致させるために使う
fn pointer_escape(s: &str) -> String {
    s.replace('~', "~0").replace('/', "~1")
}

/// Value を注記用のプレーン文字列にする（文字列は引用符なし・複合値はコンパクト JSON）
fn plain_value(v: &Value) -> String {
    match v {
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.clone(),
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::super::{NoFiles, SpecKind, render_spec};

    /// 最小 OpenAPI: 1 path・GET・パラメータ 1・レスポンス 200
    #[test]
    fn 最小_openapi_でメソッドバッジとパスとテーブルが出る() {
        let src = r#"
openapi: 3.0.3
info:
  title: サンプル API
  version: "1.0.0"
  description: テスト用
paths:
  /users/{id}:
    get:
      summary: ユーザ取得
      parameters:
        - name: id
          in: path
          required: true
          description: ユーザ ID
          schema:
            type: integer
      responses:
        "200":
          description: 成功
          content:
            application/json:
              schema:
                type: object
                properties:
                  name:
                    type: string
"#;
        let html = render_spec(SpecKind::OpenApi, src, None, &NoFiles);
        assert!(html.contains("<section class=\"api-spec\">"));
        assert!(html.contains("api-method api-method-get"));
        assert!(html.contains(">GET<"));
        assert!(html.contains("<code>/users/{id}</code>"));
        assert!(html.contains("ユーザ取得"));
        // パラメータテーブル
        assert!(html.contains("<th>名前</th>"));
        assert!(html.contains("<code>id</code>"));
        assert!(html.contains("<code>path</code>"));
        // レスポンス
        assert!(html.contains("api-status-2xx"));
        assert!(html.contains(">200<"));
        // レスポンスボディのプロパティ
        assert!(html.contains("<code>name</code>"));
        assert!(html.ends_with("</section>\n"));
    }

    /// $ref 解決（components/schemas 参照）
    #[test]
    fn ローカル_ref_を解決して参照先を描画する() {
        let src = r##"
openapi: "3.1.0"
info: { title: T, version: "1" }
paths:
  /a:
    get:
      responses:
        "200":
          description: ok
          content:
            application/json:
              schema:
                $ref: "#/components/schemas/User"
components:
  schemas:
    User:
      type: object
      required: [id]
      properties:
        id:
          type: string
        name:
          type: string
"##;
        let html = render_spec(SpecKind::OpenApi, src, None, &NoFiles);
        // 参照先の型名が「型」列に出る
        assert!(html.contains("<code>User</code>"));
        // 展開されて User のプロパティが出る
        assert!(html.contains("<code>id</code>"));
        assert!(html.contains("<code>name</code>"));
        // required の ✓
        assert!(html.contains("✓"));
    }

    /// 循環 $ref（自己参照）でも無限ループしない
    #[test]
    fn 循環_ref_はガードされ無限ループしない() {
        let src = r##"
openapi: "3.0.0"
info: { title: T, version: "1" }
paths:
  /a:
    get:
      responses:
        "200":
          description: ok
          content:
            application/json:
              schema:
                $ref: "#/components/schemas/Node"
components:
  schemas:
    Node:
      type: object
      properties:
        value:
          type: string
        next:
          $ref: "#/components/schemas/Node"
"##;
        let html = render_spec(SpecKind::OpenApi, src, None, &NoFiles);
        // ここまで到達すれば無限ループしていない
        assert!(html.contains("<code>Node</code>"));
        assert!(html.contains("<code>value</code>"));
        // 自己参照プロパティの型名は出るが、それ以上は展開されない
        assert!(html.contains("<code>next</code>"));
    }

    /// enum・配列・format・required の表示
    #[test]
    fn enum_配列_format_required_が表示される() {
        let src = r#"
type: object
required: [status]
properties:
  status:
    type: string
    enum: [active, inactive]
  createdAt:
    type: string
    format: date-time
  tags:
    type: array
    items:
      type: string
"#;
        let html = render_spec(SpecKind::JsonSchema, src, None, &NoFiles);
        // enum の値
        assert!(html.contains("<code>active</code>"));
        assert!(html.contains("<code>inactive</code>"));
        // format は型列に併記
        assert!(html.contains("string (date-time)"));
        // 配列は Type[]
        assert!(html.contains("string[]"));
        // required ✓
        assert!(html.contains("✓"));
    }

    /// JSON 入力でも YAML 入力でも同一出力
    #[test]
    fn json_入力と_yaml_入力で同一出力になる() {
        let yaml = r#"
type: object
required: [a]
properties:
  a:
    type: string
  b:
    type: integer
"#;
        let json = r#"
{
  "type": "object",
  "required": ["a"],
  "properties": {
    "a": { "type": "string" },
    "b": { "type": "integer" }
  }
}
"#;
        let from_yaml = render_spec(SpecKind::JsonSchema, yaml, None, &NoFiles);
        let from_json = render_spec(SpecKind::JsonSchema, json, None, &NoFiles);
        assert_eq!(from_yaml, from_json);
    }

    /// JSON Schema 単体ブロック
    #[test]
    fn jsonschema_単体で_title_と_schema_が出る() {
        let src = r#"
title: 住所
description: 郵送先
type: object
properties:
  zip:
    type: string
  city:
    type: string
"#;
        let html = render_spec(SpecKind::JsonSchema, src, None, &NoFiles);
        assert!(html.contains("<section class=\"api-spec api-schema\">"));
        assert!(html.contains("<strong>住所</strong>"));
        assert!(html.contains("郵送先"));
        assert!(html.contains("<code>zip</code>"));
        assert!(html.contains("<code>city</code>"));
        // スキーマ一覧は OpenAPI 文書専用（JSON Schema 単体には出ない）
        assert!(!html.contains("api-schemas"));
    }

    /// パース失敗 → エラーボックス
    #[test]
    fn パース失敗はエラーボックスになる() {
        let src = "openapi: 3.0.0\n  : : invalid : :\n\t bad";
        let html = render_spec(SpecKind::OpenApi, src, None, &NoFiles);
        assert!(html.contains("markdown-alert-caution"));
    }

    /// Swagger 2.0 の最小文書が描画される（裸の `swagger: 2.0` も Number → "2.0" で受理）
    #[test]
    fn swagger_2_0_が描画される() {
        let src = r#"
swagger: 2.0
info: { title: 旧API, version: "1" }
paths:
  /items:
    get:
      summary: 一覧
      responses:
        "200":
          description: 成功
"#;
        let html = render_spec(SpecKind::OpenApi, src, None, &NoFiles);
        assert!(!html.contains("markdown-alert-caution"), "{html}");
        assert!(html.contains("<section class=\"api-spec\">"));
        assert!(html.contains("api-method api-method-get"));
        assert!(html.contains("api-status-2xx"));
    }

    /// openapi / swagger のどちらでもない文書はエラーボックス（三分岐の else を固定）
    #[test]
    fn バージョン不明はエラーボックスになる() {
        let src = "info: { title: 不明, version: \"1\" }\npaths: {}\n";
        let html = render_spec(SpecKind::OpenApi, src, None, &NoFiles);
        assert!(html.contains("markdown-alert-caution"));
        assert!(html.contains("OpenAPI 3.x / Swagger 2.0 のみ対応"));
    }

    /// 2.0 の `in: body` はリクエストボディとして出て、パラメータ表には出ない
    #[test]
    fn swagger_2_0_の_body_パラメータはリクエストボディになる() {
        let src = r#"
swagger: "2.0"
info: { title: T, version: "1" }
paths:
  /users:
    post:
      consumes: [application/json]
      parameters:
        - name: payload
          in: body
          required: true
          description: 登録内容
          schema:
            type: object
            properties:
              name:
                type: string
      responses:
        "201":
          description: 作成
"#;
        let html = render_spec(SpecKind::OpenApi, src, None, &NoFiles);
        assert!(html.contains("リクエストボディ"), "{html}");
        assert!(html.contains("api-required-note"));
        assert!(html.contains("登録内容"));
        assert!(html.contains("<code>application/json</code>"));
        assert!(html.contains("<code>name</code>"));
        // body パラメータはパラメータ表に混ざらない（表自体が出ない）
        assert!(!html.contains("<th>場所</th>"), "{html}");
    }

    /// 2.0 の非 body パラメータは型情報を直下に持つ（type/enum が表に出る）
    #[test]
    fn swagger_2_0_のパラメータ直下の型が表に出る() {
        let src = r#"
swagger: "2.0"
info: { title: T, version: "1" }
paths:
  /items:
    get:
      parameters:
        - name: sort
          in: query
          type: string
          enum: [asc, desc]
      responses:
        "200":
          description: 成功
"#;
        let html = render_spec(SpecKind::OpenApi, src, None, &NoFiles);
        assert!(html.contains("<code>sort</code>"));
        assert!(html.contains("<code>query</code>"));
        // 型はパラメータ直下の type から、enum は注記チップとして出る
        assert!(html.contains("<code>string</code>"), "{html}");
        assert!(html.contains("<code>asc</code>"), "{html}");
        // 3.x では schema 無しパラメータは従来どおり「—」のまま（フォールバックしない）
        let v3 = r#"
openapi: "3.0.0"
info: { title: T, version: "1" }
paths:
  /items:
    get:
      parameters:
        - name: raw
          in: query
      responses:
        "200":
          description: ok
"#;
        let html_v3 = render_spec(SpecKind::OpenApi, v3, None, &NoFiles);
        assert!(html_v3.contains("<code>—</code>"), "{html_v3}");
    }

    /// produces は operation 側が top-level を上書きする
    #[test]
    fn swagger_2_0_の_produces_は_operation_が優先される() {
        let src = r#"
swagger: "2.0"
info: { title: T, version: "1" }
produces: [application/xml]
paths:
  /a:
    get:
      produces: [application/json]
      responses:
        "200":
          description: ok
          schema:
            type: string
  /b:
    get:
      responses:
        "200":
          description: ok
          schema:
            type: string
"#;
        let html = render_spec(SpecKind::OpenApi, src, None, &NoFiles);
        // /a は op 側の application/json（top-level の xml は出ない…は /b で出るので回数で確認）
        assert!(html.contains("<code>application/json</code>"));
        // /b は top-level の application/xml が既定として出る
        assert!(html.contains("<code>application/xml</code>"));
        assert_eq!(html.matches("application/xml").count(), 1, "{html}");
    }

    /// `#/definitions/...` の $ref が解決・展開される
    #[test]
    fn swagger_2_0_の_definitions_ref_を解決する() {
        let src = r##"
swagger: "2.0"
info: { title: T, version: "1" }
paths:
  /users/{id}:
    get:
      parameters:
        - name: id
          in: path
          type: integer
      responses:
        "200":
          description: 成功
          schema:
            $ref: "#/definitions/User"
definitions:
  User:
    type: object
    required: [name]
    properties:
      name:
        type: string
"##;
        let html = render_spec(SpecKind::OpenApi, src, None, &NoFiles);
        assert!(html.contains("<code>User</code>"), "{html}");
        assert!(html.contains("<code>name</code>"));
        assert!(html.contains("✓"), "required マーク: {html}");
    }

    /// components/schemas 一覧: 未参照スキーマも出て、$ref 展開と共存する
    #[test]
    fn components_schemas_一覧が描画される() {
        let src = r##"
openapi: "3.0.0"
info: { title: T, version: "1" }
paths:
  /users:
    get:
      responses:
        "200":
          description: ok
          content:
            application/json:
              schema:
                $ref: "#/components/schemas/User"
components:
  schemas:
    User:
      description: 利用者
      type: object
      properties:
        name:
          type: string
    Unused:
      type: object
      properties:
        orphanField:
          type: integer
"##;
        let html = render_spec(SpecKind::OpenApi, src, None, &NoFiles);
        assert!(html.contains("api-schemas"), "{html}");
        assert!(html.contains("スキーマ"));
        // 未参照スキーマも閉じた details で出る
        assert!(
            html.contains(
                "<details class=\"api-schema-def\"><summary><code>Unused</code></summary>"
            )
        );
        assert!(html.contains("<code>orphanField</code>"));
        // 参照済みスキーマは操作側のインライン展開と一覧の両方に出る（共存）
        assert!(html.contains("<summary><code>User</code></summary>"));
        assert!(html.contains("利用者"));
        assert!(html.matches("<code>name</code>").count() >= 2, "{html}");
    }

    /// 一覧内の自己参照スキーマは循環表示になり無限ループしない
    #[test]
    fn 一覧の自己参照スキーマは循環表示で終了する() {
        let src = r##"
openapi: "3.0.0"
info: { title: T, version: "1" }
paths: {}
components:
  schemas:
    Node:
      type: object
      properties:
        next:
          $ref: "#/components/schemas/Node"
"##;
        let html = render_spec(SpecKind::OpenApi, src, None, &NoFiles);
        assert!(html.contains("<summary><code>Node</code></summary>"));
        assert!(html.contains("循環参照"), "{html}");
    }

    /// components が空・不在なら一覧セクション自体を出さない
    #[test]
    fn 空の_components_では一覧が出ない() {
        for src in [
            "openapi: \"3.0.0\"\ninfo: { title: T, version: \"1\" }\npaths: {}\n",
            "openapi: \"3.0.0\"\ninfo: { title: T, version: \"1\" }\npaths: {}\ncomponents: {}\n",
            "openapi: \"3.0.0\"\ninfo: { title: T, version: \"1\" }\npaths: {}\ncomponents:\n  schemas: {}\n",
        ] {
            let html = render_spec(SpecKind::OpenApi, src, None, &NoFiles);
            assert!(!html.contains("api-schemas"), "{src}");
        }
    }

    /// 2.0 の definitions も一覧描画される
    #[test]
    fn swagger_2_0_の_definitions_一覧が描画される() {
        let src = r#"
swagger: "2.0"
info: { title: T, version: "1" }
paths: {}
definitions:
  Legacy:
    type: object
    properties:
      code:
        type: string
"#;
        let html = render_spec(SpecKind::OpenApi, src, None, &NoFiles);
        assert!(html.contains("api-schemas"), "{html}");
        assert!(html.contains("<summary><code>Legacy</code></summary>"));
        assert!(html.contains("<code>code</code>"));
    }

    /// XSS: title 等に含まれる `<script>` がエスケープされる
    #[test]
    fn 危険な文字列はエスケープされる() {
        let src = r#"
openapi: "3.0.0"
info:
  title: "<script>alert(1)</script>"
  version: "1"
paths:
  "/x<img>":
    get:
      summary: "<b>bad</b>"
      responses:
        "200":
          description: "<i>ok</i>"
"#;
        let html = render_spec(SpecKind::OpenApi, src, None, &NoFiles);
        assert!(!html.contains("<script>alert(1)</script>"));
        assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
        assert!(!html.contains("<b>bad</b>"));
        assert!(html.contains("&lt;b&gt;bad&lt;/b&gt;"));
        assert!(!html.contains("/x<img>"));
    }

    /// oneOf の列挙とリモート $ref・深さ上限の穏当な扱い
    #[test]
    fn oneof_の列挙とリモート_ref_の扱い() {
        let src = r#"
oneOf:
  - type: string
  - $ref: "https://example.com/schema.json#/Foo"
"#;
        let html = render_spec(SpecKind::JsonSchema, src, None, &NoFiles);
        assert!(html.contains("いずれか（oneOf）"));
        // リモート参照は未対応表示に留める
        assert!(html.contains("未対応の参照"));
    }

    /// root がスカラ型の jsonschema では description が 1 回だけ出る
    #[test]
    fn スカラルートの_description_は二重にならない() {
        let src = r#"
title: 識別子
description: 一意な文字列
type: string
"#;
        let html = render_spec(SpecKind::JsonSchema, src, None, &NoFiles);
        assert_eq!(
            html.matches("一意な文字列").count(),
            1,
            "render_scalar 側の 1 回だけ:\n{html}"
        );
        // object ルートは従来どおり文書ヘッダ側が出す
        // （「説明」はプロパティ表の <th> と衝突するため一意な文言で数える）
        let src = "title: T\ndescription: オブジェクト全体の説明文\ntype: object\nproperties:\n  a:\n    type: string\n";
        let html = render_spec(SpecKind::JsonSchema, src, None, &NoFiles);
        assert_eq!(html.matches("オブジェクト全体の説明文").count(), 1);
    }

    /// trace オペレーションもメソッドバッジ付きで描画される
    #[test]
    fn trace_メソッドが描画される() {
        let src = r#"
openapi: "3.1.0"
info: { title: T, version: "1" }
paths:
  /debug:
    trace:
      summary: トレース
      responses:
        "200":
          description: ok
"#;
        let html = render_spec(SpecKind::OpenApi, src, None, &NoFiles);
        assert!(html.contains("api-method-trace"), "{html}");
        assert!(html.contains(">TRACE<"));
    }

    /// テスト用: メモリ上のファイル群（キー = 正規化済みルート相対パス）
    struct MapFiles(std::collections::HashMap<String, String>);

    impl super::super::SpecFiles for MapFiles {
        fn read(&self, rel: &str) -> Result<String, String> {
            self.0
                .get(rel)
                .cloned()
                .ok_or_else(|| format!("{rel} が見つかりません"))
        }
    }

    fn map_files(entries: &[(&str, &str)]) -> MapFiles {
        MapFiles(
            entries
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        )
    }

    /// ファイル $ref（インラインブロック = ルート相対）で別ファイルのスキーマが展開される
    #[test]
    fn ファイル_ref_で別ファイルのスキーマが展開される() {
        let files = map_files(&[(
            "schemas/common.yaml",
            "components:\n  schemas:\n    User:\n      type: object\n      required: [id]\n      properties:\n        id:\n          type: string\n",
        )]);
        let src = r#"
openapi: "3.1.0"
info: { title: T, version: "1" }
paths:
  /users:
    get:
      responses:
        "200":
          description: ok
          content:
            application/json:
              schema:
                $ref: "schemas/common.yaml#/components/schemas/User"
"#;
        let html = render_spec(SpecKind::OpenApi, src, None, &files);
        assert!(html.contains("<code>User</code>"), "{html}");
        assert!(html.contains("<code>id</code>"), "参照先が展開される");
        assert!(!html.contains("markdown-alert-caution"));
    }

    /// 参照先ファイル内のローカル `#/...` はそのファイルの root で解決される
    #[test]
    fn 参照先ファイル内のローカル_ref_はそのファイルの_root_で解決される() {
        let files = map_files(&[(
            "common.yaml",
            concat!(
                "components:\n  schemas:\n",
                "    User:\n      type: object\n      properties:\n        address:\n          $ref: \"#/components/schemas/Address\"\n",
                "    Address:\n      type: object\n      properties:\n        zip:\n          type: string\n",
            ),
        )]);
        // メイン文書（インライン）には Address が無い = common.yaml 側で解決された証拠
        let src = "$ref: \"common.yaml#/components/schemas/User\"\n";
        let html = render_spec(SpecKind::JsonSchema, src, None, &files);
        assert!(html.contains("<code>address</code>"), "{html}");
        assert!(
            html.contains("<code>zip</code>"),
            "ネスト解決が正しい root で行われる"
        );
    }

    /// 仕様ファイル内の相対 $ref は参照元ファイルのディレクトリ基準で解決される
    #[test]
    fn 仕様ファイル内の相対_ref_は参照元ディレクトリ基準() {
        let files = map_files(&[(
            // specs/api.yaml（origin）から "schemas/x.yaml" → specs/schemas/x.yaml
            "specs/schemas/x.yaml",
            "type: object\nproperties:\n  ok:\n    type: string\n",
        )]);
        let src = "$ref: \"schemas/x.yaml\"\n"; // fragment なし = 文書全体
        let html = render_spec(SpecKind::JsonSchema, src, Some("specs/api.yaml"), &files);
        assert!(html.contains("<code>ok</code>"), "{html}");
    }

    /// ファイル間の循環 $ref（a→b→a）はガードされる
    #[test]
    fn ファイル間の循環_ref_はガードされる() {
        let files = map_files(&[
            (
                "a.yaml",
                "A:\n  type: object\n  properties:\n    next:\n      $ref: \"b.yaml#/B\"\n",
            ),
            (
                "b.yaml",
                "B:\n  type: object\n  properties:\n    prev:\n      $ref: \"a.yaml#/A\"\n",
            ),
        ]);
        let src = "$ref: \"a.yaml#/A\"\n";
        let html = render_spec(SpecKind::JsonSchema, src, None, &files);
        // ここまで到達すれば無限ループ・無限ロードしていない
        assert!(html.contains("<code>next</code>"), "{html}");
        assert!(html.contains("循環参照"), "戻り参照は循環と表示される");
    }

    /// ルート外への `../` 参照は正規化段で拒否され注記になる
    #[test]
    fn ルート外への_ref_は拒否され注記になる() {
        let files = map_files(&[]);
        let src = "$ref: \"../outside.yaml#/X\"\n";
        let html = render_spec(SpecKind::JsonSchema, src, None, &files);
        assert!(html.contains("ルートの外"), "{html}");
        assert!(
            !html.contains("markdown-alert-caution"),
            "ブロック全体は生きる"
        );
    }

    /// 不在ファイル・参照先のパース失敗は注記で描画継続する（error_box にしない）
    #[test]
    fn 不在ファイルとパース失敗は注記で描画継続() {
        let files = map_files(&[("broken.yaml", "foo: [unclosed")]);
        let src = concat!(
            "type: object\nproperties:\n",
            "  a:\n    $ref: \"missing.yaml#/X\"\n",
            "  b:\n    $ref: \"broken.yaml#/X\"\n",
        );
        let html = render_spec(SpecKind::JsonSchema, src, None, &files);
        assert!(html.contains("読み込み失敗"), "{html}");
        assert!(html.contains("<code>a</code>"), "残りは描画される");
        assert!(html.contains("<code>b</code>"));
        assert!(!html.contains("markdown-alert-caution"));
    }

    /// 出力の構造がおおむね well-formed（section とタグの対応）
    #[test]
    fn 出力の主要タグが対応している() {
        let src = r#"
openapi: "3.0.0"
info: { title: T, version: "1" }
paths:
  /a:
    get:
      responses:
        "200":
          description: ok
"#;
        let html = render_spec(SpecKind::OpenApi, src, None, &NoFiles);
        assert_eq!(html.matches("<section").count(), 1);
        assert_eq!(html.matches("</section>").count(), 1);
        assert_eq!(
            html.matches("<details").count(),
            html.matches("</details>").count()
        );
    }
}
