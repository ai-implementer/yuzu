//! OpenAPI 3.x / JSON Schema の Value 走査 → HTML 組み立て（[`super::render_spec`] の本体）。
//!
//! 設計:
//! - 入力（YAML / JSON）は `serde_yaml_ng` で `serde_json::Value` に読む。
//!   yuzu-render の serde_json は `preserve_order` 有効なので Map は記述順を保つ
//!   （= 出力は決定的。HashMap の非決定順序を混ぜない）
//! - 走査中の全テキストは [`escape_html`] を通してから埋め込む（XSS 安全）
//! - `$ref` はローカル（`#/...`）のみ解決。訪問スタック `stack` で循環をガードし、
//!   循環・未解決・リモート参照は本文の外に注記を出すに留める

use serde_json::Value;

use crate::highlight::escape_html as esc;

use super::SpecKind;

/// 入れ子スキーマ描画の深さ上限（これを超えたら「以降省略」）
const MAX_DEPTH: usize = 8;

/// スキーマ合成キーワードと、そのセクション見出しラベル
const COMBINATORS: [(&str, &str); 3] = [
    ("oneOf", "いずれか（oneOf）"),
    ("anyOf", "いずれか（anyOf）"),
    ("allOf", "すべて（allOf）"),
];

/// 仕様テキストを HTML に変換する（[`super::render_spec`] から委譲される本体）
pub(super) fn render(kind: SpecKind, source: &str) -> String {
    let value: Value = match serde_yaml_ng::from_str(source) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(error = %e, "API 仕様のパースに失敗しました");
            return super::error_box(&format!("パースに失敗しました: {e}"), source);
        }
    };

    match kind {
        SpecKind::OpenApi => {
            // openapi フィールドが 3 で始まらない（Swagger 2.0 等）なら未対応
            let is_v3 = value
                .get("openapi")
                .map(plain_value)
                .is_some_and(|v| v.starts_with('3'));
            if !is_v3 {
                let msg = "OpenAPI 3.x のみ対応しています（`openapi: 3.x.y` が必要です）";
                tracing::warn!("{msg}");
                return super::error_box(msg, source);
            }
            let mut r = Renderer::new(&value);
            r.render_openapi_document(&value)
        }
        SpecKind::JsonSchema => {
            let mut r = Renderer::new(&value);
            r.render_jsonschema_document(&value)
        }
    }
}

/// 走査状態。`root` は `$ref` 解決の基点、`stack` は解決中の参照ポインタ（循環ガード）
struct Renderer<'a> {
    root: &'a Value,
    stack: Vec<String>,
}

impl<'a> Renderer<'a> {
    fn new(root: &'a Value) -> Self {
        Self {
            root,
            stack: Vec::new(),
        }
    }

    // ---- 文書レベル ----

    fn render_openapi_document(&mut self, root: &'a Value) -> String {
        let mut out = String::from("<section class=\"api-spec\">\n");
        out.push_str(&self.render_info(root.get("info")));
        if let Some(paths) = root.get("paths").and_then(|v| v.as_object()) {
            out.push_str("<div class=\"api-paths\">\n");
            for (path, item) in paths {
                out.push_str(&self.render_path_item(path, item));
            }
            out.push_str("</div>\n");
        }
        out.push_str("</section>\n");
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
        if let Some(d) = root.get("description").and_then(|v| v.as_str()) {
            out.push_str(&format!("<p class=\"api-desc\">{}</p>\n", esc(d)));
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
        let item = self.deref(item);
        let path_params = item.get("parameters").and_then(|v| v.as_array());
        let mut out = String::new();
        for method in ["get", "put", "post", "delete", "patch", "head", "options"] {
            if let Some(op) = item.get(method) {
                out.push_str(&self.render_operation(path, method, op, path_params));
            }
        }
        out
    }

    fn render_operation(
        &mut self,
        path: &str,
        method: &str,
        op: &'a Value,
        path_params: Option<&'a Vec<Value>>,
    ) -> String {
        let op = self.deref(op);
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
        let merged = self.merge_parameters(path_params, op_params);
        if !merged.is_empty() {
            out.push_str(&self.render_parameters(&merged));
        }
        if let Some(rb) = op.get("requestBody") {
            out.push_str(&self.render_request_body(rb, 0));
        }
        if let Some(resp) = op.get("responses") {
            out.push_str(&self.render_responses(resp, 0));
        }
        out.push_str("</details>\n");
        out
    }

    /// path-item レベルと operation レベルの parameters をマージする。
    /// 同名・同 in の重複は operation を優先し、path-item 側を落とす
    fn merge_parameters(
        &self,
        path_params: Option<&'a Vec<Value>>,
        op_params: Option<&'a Vec<Value>>,
    ) -> Vec<&'a Value> {
        let op: Vec<&Value> = op_params
            .map(|a| a.iter().map(|p| self.deref(p)).collect())
            .unwrap_or_default();
        let op_keys: Vec<(String, String)> = op.iter().map(|p| param_key(p)).collect();

        let mut result: Vec<&Value> = Vec::new();
        if let Some(pp) = path_params {
            for p in pp {
                let p = self.deref(p);
                let key = param_key(p);
                if op_keys.contains(&key) {
                    continue; // operation 側で上書きされる
                }
                result.push(p);
            }
        }
        result.extend(op);
        result
    }

    fn render_parameters(&self, params: &[&'a Value]) -> String {
        let mut out = String::from(
            "<div class=\"api-params\"><p class=\"api-section-label\">パラメータ</p>\n\
             <table class=\"api-schema-table\">\n\
             <thead><tr><th>名前</th><th>場所</th><th>型</th><th>必須</th><th>説明</th></tr></thead>\n\
             <tbody>\n",
        );
        for p in params {
            let name = p.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let loc = p.get("in").and_then(|v| v.as_str()).unwrap_or("");
            let schema = p.get("schema");
            let ty = schema.map_or_else(|| "—".to_string(), |s| self.type_label(s));
            // path パラメータは仕様上つねに必須
            let required =
                p.get("required").and_then(|v| v.as_bool()) == Some(true) || loc == "path";
            let req_mark = if required { "✓" } else { "" };

            let mut desc = String::new();
            if let Some(d) = p.get("description").and_then(|v| v.as_str()) {
                desc.push_str(&esc(d));
            }
            if let Some(s) = schema {
                desc.push_str(&self.annotations_html(s));
            }

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
        let rb = self.deref(rb);
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
            out.push_str(&self.render_content(content, depth));
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
            let r = self.deref(r);
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
                out.push_str(&self.render_content(content, depth));
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

    /// `$ref` を解決して描画する。ローカル参照のみ・循環はスタックで止める
    fn render_ref(&mut self, r: &str, depth: usize) -> String {
        if !is_local_ref(r) {
            return format!(
                "<p class=\"api-ref\">{}（未対応の参照）</p>\n",
                code(ref_name(r))
            );
        }
        if self.stack.iter().any(|s| s == r) {
            return format!(
                "<p class=\"api-ref\">{}（循環参照）</p>\n",
                code(ref_name(r))
            );
        }
        match self.resolve_local_ref(r) {
            Some(target) => {
                self.stack.push(r.to_string());
                let body = self.render_schema(target, depth);
                self.stack.pop();
                // どのスキーマを展開したかを見出しに示す
                format!(
                    "<p class=\"api-ref-name\">{}</p>\n{body}",
                    code(ref_name(r))
                )
            }
            None => format!(
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
    /// 解決可能で循環しないローカル `$ref` のみ true）
    fn should_expand(&self, schema: &Value) -> bool {
        if let Some(r) = ref_str(schema) {
            if !is_local_ref(r) || self.stack.iter().any(|s| s == r) {
                return false;
            }
            return self.resolve_local_ref(r).is_some();
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
            if !is_local_ref(r) {
                return format!("{name}（未対応の参照）");
            }
            if self.stack.iter().any(|s| s == r) {
                return format!("{name}（循環参照）");
            }
            if self.resolve_local_ref(r).is_none() {
                return format!("{name}（未解決の参照）");
            }
            return name.to_string();
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

    /// ローカル JSON ポインタ（`#/a/b`）を解決する。RFC6901 のデコードは pointer が担う
    fn resolve_local_ref(&self, r: &str) -> Option<&'a Value> {
        let pointer = r.strip_prefix('#')?;
        self.root.pointer(pointer)
    }

    /// パラメータ / requestBody / response ラッパの `$ref` を実体まで剥がす
    fn deref(&self, v: &'a Value) -> &'a Value {
        let mut cur = v;
        for _ in 0..MAX_DEPTH {
            match ref_str(cur) {
                Some(r) if is_local_ref(r) => match self.resolve_local_ref(r) {
                    Some(t) => cur = t,
                    None => return cur,
                },
                _ => return cur,
            }
        }
        cur
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

/// ローカル参照（`#/...`）か
fn is_local_ref(r: &str) -> bool {
    r.starts_with('#')
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
    use super::super::{SpecKind, render_spec};

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
        let html = render_spec(SpecKind::OpenApi, src);
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
        let html = render_spec(SpecKind::OpenApi, src);
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
        let html = render_spec(SpecKind::OpenApi, src);
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
        let html = render_spec(SpecKind::JsonSchema, src);
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
        let from_yaml = render_spec(SpecKind::JsonSchema, yaml);
        let from_json = render_spec(SpecKind::JsonSchema, json);
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
        let html = render_spec(SpecKind::JsonSchema, src);
        assert!(html.contains("<section class=\"api-spec api-schema\">"));
        assert!(html.contains("<strong>住所</strong>"));
        assert!(html.contains("郵送先"));
        assert!(html.contains("<code>zip</code>"));
        assert!(html.contains("<code>city</code>"));
    }

    /// パース失敗 → エラーボックス
    #[test]
    fn パース失敗はエラーボックスになる() {
        let src = "openapi: 3.0.0\n  : : invalid : :\n\t bad";
        let html = render_spec(SpecKind::OpenApi, src);
        assert!(html.contains("markdown-alert-caution"));
    }

    /// Swagger 2.0 → 未対応メッセージ
    #[test]
    fn swagger_2_0_は未対応メッセージになる() {
        let src = r#"
swagger: "2.0"
info: { title: 旧API, version: "1" }
paths: {}
"#;
        let html = render_spec(SpecKind::OpenApi, src);
        assert!(html.contains("markdown-alert-caution"));
        assert!(html.contains("OpenAPI 3.x のみ対応"));
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
        let html = render_spec(SpecKind::OpenApi, src);
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
        let html = render_spec(SpecKind::JsonSchema, src);
        assert!(html.contains("いずれか（oneOf）"));
        // リモート参照は未対応表示に留める
        assert!(html.contains("未対応の参照"));
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
        let html = render_spec(SpecKind::OpenApi, src);
        assert_eq!(html.matches("<section").count(), 1);
        assert_eq!(html.matches("</section>").count(), 1);
        assert_eq!(
            html.matches("<details").count(),
            html.matches("</details>").count()
        );
    }
}
