//! comrak を使う唯一のモジュール（外部へは型を漏らさない）。
//!
//! - パス1: [`extract_meta`] — frontmatter / 先頭 h1 / TOC（全見出し）を抽出
//! - パス2: [`render_body_html`] — コードブロック差し替え・URL 書き換えを
//!   AST 上で行ってから HTML 化
//!
//! ⚠️ アンカー ID の同期: comrak の `header_ids` 拡張は HTML 化時に内部の
//! `Anchorizer` で ID を採番する。TOC 側も**全見出しを文書順で**採番することで
//! 重複サフィックス（`-1` 等）を一致させている。片方だけ見出しを飛ばすとずれる。

pub(crate) mod collapse;
pub(crate) mod crossref;
pub(crate) mod fence;
pub(crate) mod fragment;
pub(crate) mod glossary;
pub(crate) mod suppress_comment;
pub(crate) mod tabs;

/// 属性・テキストへ埋める文字列の HTML エスケープ。
/// 本文 HTML を組み立てる collapse / crossref / tabs / glossary が共有する
/// （同じ規則を複数実装するとエスケープ漏れが片方だけ起きる）
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

use std::path::Path;

use comrak::nodes::{AstNode, NodeHtmlBlock, NodeValue};
use comrak::{Anchorizer, Arena, Options, format_commonmark, format_html, parse_document};

use crate::MarkdownOptions;
use crate::error::CoreError;
use crate::frontmatter::parse_frontmatter;
use crate::markdown::fence::parse_fence_info;
use crate::model::{CrossrefLabel, Frontmatter, Page, PlainSection, SourceSpan, TocEntry};
use crate::traits::{CodeBlockRenderer, UrlRewriter};

/// comrak のオプションを組み立てる（凍結: GFM 拡張＋YAML frontmatter＋header_ids）。
///
/// - AST ノードの sourcepos は常に記録されるため `render.sourcepos` は不要
///   （HTML に `data-sourcepos` 属性を撒かない）
/// - `render.unsafe_ = true` はコードブロック差し替え（HtmlBlock の素通し）と
///   著者の生 HTML のため。docs は信頼できる入力という前提
fn comrak_options(opts: &MarkdownOptions) -> Options<'static> {
    let mut options = Options::default();
    if opts.gfm {
        options.extension.table = true;
        options.extension.strikethrough = true;
        options.extension.autolink = true;
        options.extension.tasklist = true;
        options.extension.alerts = true; // > [!NOTE] 等の Admonition（GitHub 互換 5 種）
        options.extension.footnotes = true; // [^name] 脚注
    }
    if opts.math {
        options.extension.math_dollars = true; // $...$ / $$...$$（通貨 $100 等は弾かれる）
        options.extension.math_code = true; // $`...`$（```math フェンスは CodeBlock のまま）
    }
    options.extension.front_matter_delimiter = Some("---".to_string());
    options.extension.header_id_prefix = Some(String::new());
    // 日本語の約物に隣接した強調（`**この文は重要です。**但し…`）。CommonMark の
    // flanking 規則では閉じ `**` が「約物に前置・非約物に後続」だと成立せず、
    // `**「重要」**です` のような日本語で頻出する形が素通しになる
    options.extension.cjk_friendly_emphasis = true;
    // 定義リスト（`用語` → 空行 → `: 説明` → <dl>/<dt>/<dd>）。
    // format_commonmark も対応しているので `yuzu fmt` の往復が成立する
    options.extension.description_lists = true;
    options.render.r#unsafe = true;
    options
}

/// fmt / normalize / linkcheck 用: 脚注定義を**ソース位置のまま**温存する。
///
/// comrak は既定でパース終端に「参照済み定義を文書末尾へ移動・未参照定義を削除」
/// する（process_footnotes）。fmt のバイト尊重方針と衝突するため、整形・正規化・
/// リンク検査は定義を動かさないこのオプションでパースする。
///
/// ⚠️ HTML レンダに使ってはならない: `<section class="footnotes">` ラッパが
/// 最初の定義位置で 1 回しか開かれず HTML が壊れる。
/// ⚠️ `extract_meta` にも使わない: 見出しのアンカー採番順が render とずれる
fn comrak_options_keep_footnotes(opts: &MarkdownOptions) -> Options<'static> {
    let mut options = comrak_options(opts);
    options.parse.leave_footnote_definitions = true;
    options
}

/// パス1 の結果
pub(crate) struct ExtractedMeta {
    pub frontmatter: Frontmatter,
    pub first_h1: Option<String>,
    pub toc: Vec<TocEntry>,
    pub labels: Vec<CrossrefLabel>,
}

/// frontmatter・先頭 h1・TOC（h1〜h6 全見出し＋アンカー ID）を抽出する
pub(crate) fn extract_meta(
    source: &str,
    opts: &MarkdownOptions,
    src_path: &Path,
) -> Result<ExtractedMeta, CoreError> {
    let arena = Arena::new();
    let options = comrak_options(opts);
    let root = parse_document(&arena, source, &options);

    let mut frontmatter = Frontmatter::default();
    let mut first_h1 = None;
    let mut toc = Vec::new();
    let mut labels = Vec::new();
    // HTML 化時の header_ids 拡張と同じ採番になるよう、全見出しを文書順で anchorize
    let mut anchorizer = Anchorizer::new();
    // 図表キャプションの採番（render_body_html 側と同じ文書順・同じ規則で回す）
    let mut numbering = crossref::Numbering::default();

    for node in root.descendants() {
        let data = node.data.borrow();
        match &data.value {
            NodeValue::Paragraph => {
                // キャプション行（`Figure: 説明 {#fig:label}`）。ラベルなしでも採番する
                if let Some(caption) = crossref::parse_caption(&collect_text(node)) {
                    let number = numbering.next(caption.kind);
                    if let Some(id) = caption.label {
                        labels.push(CrossrefLabel {
                            id,
                            kind: caption.kind,
                            number,
                            text: caption.text,
                            span: span_of(&data.sourcepos),
                        });
                    }
                }
            }
            NodeValue::FrontMatter(raw) => {
                frontmatter = parse_frontmatter(raw).map_err(|message| CoreError::Frontmatter {
                    path: src_path.to_path_buf(),
                    message,
                })?;
            }
            NodeValue::Heading(heading) => {
                let text = collect_text(node);
                let id = anchorizer.anchorize(&text);
                if heading.level == 1 && first_h1.is_none() {
                    first_h1 = Some(text.clone());
                }
                toc.push(TocEntry {
                    level: heading.level,
                    id,
                    text,
                    span: span_of(&data.sourcepos),
                });
            }
            _ => {}
        }
    }

    Ok(ExtractedMeta {
        frontmatter,
        first_h1,
        toc,
        labels,
    })
}

/// `format_commonmark` が正規化してしまう yuzu 独自記法を、書き手が書いた形へ戻す。
///
/// comrak は `#` を無条件にエスケープし（`{#fig:x}` → `{\#fig:x}`）、Admonition の
/// タイトルは必ず 1 つ空白を空けて書く（`[!NOTE]-` → `[!NOTE] -`）。どちらも
/// 解釈は変わらないが原稿の見た目が変わるため、**行末のラベルと Admonition の
/// マーカーだけ**を対象に元の形へ復元する（他の `#` エスケープは触らない）。
///
/// 抑制コメント（`<!-- yuzu-lint-… -->`）の直後には comrak が必ず空行を挿入する
/// （HtmlBlock 後の blankline）ため、その空行を落として**密着形を正規形**にする。
/// 照合（suppress.rs）は「空行を飛ばした次の内容行」で行うので、
/// どちらの形でも抑制の意味は変わらない = 見た目だけの復元
fn restore_yuzu_syntax(body: String) -> String {
    if !body.contains("{\\#")
        && !body.contains("] -")
        && !body.contains("] +")
        && !body.contains("yuzu-lint-")
    {
        return body;
    }
    let lines: Vec<&str> = body.split_inclusive('\n').collect();
    let mut out = String::with_capacity(body.len());
    let mut i = 0;
    while i < lines.len() {
        let (text, newline) = match lines[i].strip_suffix('\n') {
            Some(text) => (text, "\n"),
            None => (lines[i], ""),
        };
        out.push_str(&restore_line(text));
        out.push_str(newline);
        // 抑制コメント直後の挿入空行を落とす（次が内容行のときだけ = 文末や
        // 連続空行は触らない）。tight リスト内は comrak が空行を入れないため
        // 条件が成立せず、何もしない
        if suppress_comment::is_suppress_comment_line(text)
            && i + 2 < lines.len()
            && suppress_comment::is_content_blank(lines[i + 1])
            && !suppress_comment::is_content_blank(lines[i + 2])
        {
            i += 1;
        }
        i += 1;
    }
    out
}

/// 1 行ぶんの復元（キャプション行のラベルと Admonition の折りたたみマーカー）
fn restore_line(text: &str) -> String {
    // キャプション行の末尾ラベル: `... {\#fig:x}` → `... {#fig:x}`
    // （行末が `{\#…}` で、中身に空白を含まないものだけ）
    if let Some(head) = text.strip_suffix('}') {
        if let Some((before, label)) = head.rsplit_once("{\\#") {
            if !label.is_empty()
                && !label.contains(char::is_whitespace)
                && crossref::parse_caption(before.trim_start_matches(['>', ' '])).is_some()
            {
                return format!("{before}{{#{label}}}");
            }
        }
    }
    // Admonition の折りたたみマーカー: `> [!NOTE] - 題` → `> [!NOTE]- 題`
    if let Some(marker_at) = text.find("] ") {
        let (head, rest) = text.split_at(marker_at);
        if head.trim_start().starts_with("> [!") || head.trim_start().starts_with("[!") {
            let after = &rest[2..];
            if let Some(marker) = after.chars().next() {
                if marker == '-' || marker == '+' {
                    return format!("{head}]{after}");
                }
            }
        }
    }
    text.to_string()
}

/// 本文を HTML 化する（コードブロック差し替え・URL 書き換えつき）
/// [`render_body_html`] の結果
pub struct RenderedBody {
    /// 本文 HTML
    pub html: String,
    /// Markdown 断片（` ```include `）を参照したか（= 本文キャッシュ非対象の印。
    /// 解決の成否に依らず立てる — 参照先が後から現れたときも再描画が要るため。
    /// コード引用の `file=` は yuzu-render 側の `external_deps` が担い、
    /// こちらは core 展開ぶんを補完する。片方だけ見ると v15 の事故が再演する）
    pub used_fragment: bool,
}

pub(crate) fn render_body_html(
    page: &Page,
    opts: &MarkdownOptions,
    code: &dyn CodeBlockRenderer,
    urls: &dyn UrlRewriter,
    project_root: Option<&Path>,
) -> Result<RenderedBody, CoreError> {
    let arena = Arena::new();
    let options = comrak_options(opts);
    let root = parse_document(&arena, &page.source, &options);
    let mut used_fragment = false;

    // ── パス0: 木を触らない軽い収集（CodeBlock だけを見る）──
    // - `include` フェンス → 断片展開の対象
    // - `tab=` 付きフェンス → タブ成員
    // ⚠️ タブ判定は**展開前のこの木**で行う。lint（extract_fence_meta）も原文の木で
    // 同じ判定をするので、空断片を展開してもグループ判定が lint とずれない
    let mut fragment_nodes: Vec<(&AstNode, Option<fence::IncludeSpec>)> = Vec::new();
    let mut tab_members: Vec<(&AstNode, String)> = Vec::new();
    for node in root.descendants() {
        let data = node.data.borrow();
        if let NodeValue::CodeBlock(cb) = &data.value {
            let (lang, meta) = parse_fence_info(&cb.info);
            if lang == Some(fragment::FRAGMENT_LANG) {
                fragment_nodes.push((node, meta.include));
                continue;
            }
            if let Some(tab) = &meta.tab {
                if tabs::has_tab_neighbor(node, opts) {
                    tab_members.push((node, tab.clone()));
                }
            }
        }
    }

    // ── パス0 適用: 断片展開（走査は終わっているので構造変更が安全）──
    // 断片ノードはこの後のパス1 を通るので、URL 書き換え・断片内コードの
    // ハイライト・折りたたみは通常の本文と同じ経路で処理される
    for (node, spec) in fragment_nodes {
        let Some(spec) = spec else {
            // file= の無い ```include はエラーボックス（lint も警告する）
            node.data.borrow_mut().value = NodeValue::HtmlBlock(NodeHtmlBlock {
                block_type: 6,
                literal: fragment::error_box("file= がありません", ""),
            });
            continue;
        };
        used_fragment = true;
        let text = match project_root {
            None => {
                Err("このビルドではファイル参照が使えません（基準ディレクトリ未設定）".to_string())
            }
            Some(root) => crate::include::resolve_include(root, &spec),
        };
        match text {
            Err(message) => {
                node.data.borrow_mut().value = NodeValue::HtmlBlock(NodeHtmlBlock {
                    block_type: 6,
                    literal: fragment::error_box(&message, &spec.path),
                });
            }
            Ok(text) => {
                // 同一 arena なのでライフタイムが揃い、host の木へそのまま移せる
                let frag = parse_document(&arena, &text, &options);
                while let Some(child) = frag.first_child() {
                    child.detach();
                    // 断片先頭の frontmatter は黙って捨てる（check がエラーにする。
                    // 描画で出すと format_html は無出力・意図も不明瞭なため）
                    if matches!(child.data.borrow().value, NodeValue::FrontMatter(_)) {
                        continue;
                    }
                    node.insert_before(child);
                }
                // 空断片なら何も挿入されず include ノードが消えるだけ
                node.detach();
            }
        }
    }

    // 相互参照の解決表（`#fig:deps` → 「図 1」）。ラベルはメタ抽出時に
    // 同じ規則で採番済みなので、ここでは引くだけ
    let labels: std::collections::HashMap<&str, &CrossrefLabel> =
        page.labels.iter().map(|l| (l.id.as_str(), l)).collect();
    // サイト通し番号では先行ページまでの個数から採番を続ける（page.crossref_offset）
    let mut numbering = page.crossref_offset;

    // ⚠️ 木の構造を変える操作（子の切り離し・追加）は descendants() の
    // イテレート中に行うと comrak が "tree modified during iteration" で
    // パニックする。走査では対象と置換内容を集めるだけにして、適用は後段で行う
    let mut block_replacements = Vec::new();
    let mut ref_fills = Vec::new();
    let mut collapsibles = Vec::new();

    for node in root.descendants() {
        // 折りたたみ Admonition（`> [!NOTE]- タイトル`）→ <details> へ組み替える
        if let NodeValue::Alert(alert) = &node.data.borrow().value {
            if let Some((collapse, title)) = collapse::parse_title(alert.title.as_deref()) {
                collapsibles.push((node, collapse::open_tag(alert.alert_type, collapse, &title)));
            }
        }

        // コードブロック → フックが返した HTML（HtmlBlock）へ差し替え
        let replacement = {
            let data = node.data.borrow();
            match &data.value {
                NodeValue::CodeBlock(cb) => {
                    let (lang, meta) = parse_fence_info(&cb.info);
                    code.render(lang, &meta, &cb.literal)
                }
                // キャプション行 → 採番済みキャプション（アンカー付き）へ
                NodeValue::Paragraph => crossref::parse_caption(&collect_text(node))
                    .map(|caption| crossref::render_caption(&caption, &mut numbering)),
                _ => None,
            }
        };
        if let Some(html) = replacement {
            block_replacements.push((node, html));
            continue;
        }

        // 空テキストのラベル参照リンク `[](#fig:deps)` → 「図 1」を補完する
        // （テキストがある `[この図](#fig:deps)` はそのまま = 著者の指定を尊重）
        let fill_text = {
            let data = node.data.borrow();
            match &data.value {
                NodeValue::Link(link) if node.first_child().is_none() => link
                    .url
                    .strip_prefix('#')
                    .and_then(|frag| labels.get(frag))
                    .map(|label| format!("{} {}", label.kind.label(), label.number)),
                _ => None,
            }
        };
        if let Some(text) = fill_text {
            ref_fills.push((node, text));
        }

        // リンク・画像の URL 書き換え（値の変更だけなので走査中で安全）
        let mut data = node.data.borrow_mut();
        if let NodeValue::Link(link) | NodeValue::Image(link) = &mut data.value {
            if let Some(rewritten) = urls.rewrite(page, &link.url) {
                link.url = rewritten;
            }
        }
    }

    for (node, html) in block_replacements {
        // 子（段落のインライン群）は HtmlBlock に持たせられないので切り離す
        // （CodeBlock は元々子を持たない）
        while let Some(child) = node.first_child() {
            child.detach();
        }
        node.data.borrow_mut().value = NodeValue::HtmlBlock(NodeHtmlBlock {
            block_type: 6,
            literal: html,
        });
    }
    for (node, text) in ref_fills {
        let start = node.data.borrow().sourcepos.start;
        let child = arena.alloc(AstNode::new(std::cell::RefCell::new(
            comrak::nodes::Ast::new(NodeValue::Text(text.into()), start),
        )));
        node.append(child);
    }
    // タブ: 隣接する `tab=` 付きコードブロックを 1 グループへ束ねる。
    // 「隣接」は**兄弟として直接つながっていること**で判定するので、間に段落が
    // 挟まればそこでグループが切れる（著者が意図せず巻き込まれない）
    let mut member_idx = 0;
    let mut group_seq = 0;
    while member_idx < tab_members.len() {
        let mut end = member_idx + 1;
        while end < tab_members.len()
            && tab_members[end - 1]
                .0
                .next_sibling()
                .is_some_and(|next| std::ptr::eq(next, tab_members[end].0))
        {
            end += 1;
        }
        // 1 枚だけのタブはグループにしない（切り替え先が無く、ラベルだけが浮く）
        if end - member_idx >= 2 {
            let start = tab_members[member_idx].0.data.borrow().sourcepos.start;
            let html_block = |literal: String| {
                arena.alloc(AstNode::new(std::cell::RefCell::new(
                    comrak::nodes::Ast::new(
                        NodeValue::HtmlBlock(NodeHtmlBlock {
                            block_type: 6,
                            literal,
                        }),
                        start,
                    ),
                )))
            };
            for (i, (node, label)) in tab_members[member_idx..end].iter().enumerate() {
                if i == 0 {
                    node.insert_before(html_block(tabs::open_group(group_seq)));
                }
                node.insert_before(html_block(tabs::open_tab(group_seq, i, label)));
            }
            tab_members[end - 1]
                .0
                .insert_after(html_block(tabs::CLOSE_GROUP.to_string()));
            group_seq += 1;
        }
        member_idx = end;
    }

    // 折りたたみ: Alert ノードを「開始タグ HtmlBlock → 中身 → 終了タグ」へ
    // 展開する（comrak は details 出力を持たないため AST 上で組み替える）
    for (node, open_tag) in collapsibles {
        let start = node.data.borrow().sourcepos.start;
        let html_block = |literal: String| {
            arena.alloc(AstNode::new(std::cell::RefCell::new(
                comrak::nodes::Ast::new(
                    NodeValue::HtmlBlock(NodeHtmlBlock {
                        block_type: 6,
                        literal,
                    }),
                    start,
                ),
            )))
        };
        node.insert_before(html_block(open_tag));
        // 中身（ブロック群）を Alert の外へ順序どおり移す
        while let Some(child) = node.first_child() {
            child.detach();
            node.insert_before(child);
        }
        node.insert_before(html_block(collapse::CLOSE_TAG.to_string()));
        node.detach();
    }

    // 適用E: 用語集の略語（ページ内初出を <abbr title> で包む）。
    //
    // ⚠️ **必ず適用 A〜D の後**に走らせる。この時点でコードブロックと図表キャプション段落は
    // HtmlBlock へ差し替わっているため、`parse_caption` の再実装なしに除外が成立する
    // （キャプションで置換すると collect_text が HtmlInline を落とし、アンカー ID が
    // extract_meta / 本文 HTML / extract_plain_sections の 3 経路でずれる）。
    // A〜D はいずれも兄弟順を保つので、ここでの再帰順 = 文書順 = 初出の順になる。
    // 用語集ページ自身は置換しない（説明文の中で自分の用語が光るのは無意味）。
    // 検索結果ページ等の他の合成ページは対象のまま（本文がほぼ無いので実質無影響）
    if page.generated != Some(crate::model::GeneratedKind::Glossary) {
        if let Some(matcher) = glossary::Matcher::new(&opts.glossary) {
            let mut used = std::collections::HashSet::new();
            let mut splits = Vec::new();
            collect_abbr(root, &matcher, &mut used, &mut splits);
            for (node, pieces) in splits {
                let start = node.data.borrow().sourcepos.start;
                let alloc = |value: NodeValue| {
                    arena.alloc(AstNode::new(std::cell::RefCell::new(
                        comrak::nodes::Ast::new(value, start),
                    )))
                };
                for piece in pieces {
                    match piece {
                        glossary::Piece::Text(text) => {
                            node.insert_before(alloc(NodeValue::Text(text.into())));
                        }
                        glossary::Piece::Abbr { term, desc } => {
                            node.insert_before(alloc(NodeValue::HtmlInline(
                                glossary::abbr_open_tag(&desc),
                            )));
                            node.insert_before(alloc(NodeValue::Text(term.into())));
                            node.insert_before(alloc(NodeValue::HtmlInline(
                                glossary::ABBR_CLOSE_TAG.to_string(),
                            )));
                        }
                    }
                }
                node.detach();
            }
        }
    }

    let mut out = String::new();
    format_html(root, &options, &mut out)?;
    Ok(RenderedBody {
        html: out,
        used_fragment,
    })
}

/// 本文を正規化 Markdown として出力する（frontmatter は含めない）。
///
/// comrak の `format_commonmark` による正規化（見出し ATX 化・箇条書き `-` 統一・
/// 裸 URL の `<url>` 化等）。llms-full.txt と将来の `yuzu fmt`（Phase 6）の共通基盤。
///
/// ⚠️ `render_body_html` 後の AST（コードブロックが HtmlBlock 化済み）を
/// 流用しないこと。必ず新規パースした AST に対して行う
pub(crate) fn normalize_markdown(
    source: &str,
    opts: &MarkdownOptions,
) -> Result<String, CoreError> {
    let arena = Arena::new();
    // 脚注定義の位置・未参照定義を温存する（llms-full は原文に忠実な正規形を出す）
    let options = comrak_options_keep_footnotes(opts);
    let root = parse_document(&arena, source, &options);

    // format_commonmark は FrontMatter ノードを（区切り行込みの生テキストごと）
    // 再出力するため、AST から外す。FrontMatter は常に Document の第一子
    if let Some(first) = root.first_child() {
        if matches!(first.data.borrow().value, NodeValue::FrontMatter(_)) {
            first.detach();
        }
    }

    let mut out = String::new();
    format_commonmark(root, &options, &mut out)?;
    Ok(out)
}

/// 本文中のリンク・画像参照（linkcheck 用）
pub(crate) struct LinkRef {
    pub url: String,
    pub is_image: bool,
    pub span: SourceSpan,
}

/// 本文中のリンク・画像の URL を sourcepos 付きで列挙する（`yuzu check` 用）。
/// autolink（GFM）もリンクとして現れる
pub(crate) fn extract_link_refs(source: &str, opts: &MarkdownOptions) -> Vec<LinkRef> {
    let arena = Arena::new();
    // 既定オプションだと未参照の脚注定義が AST から消え、その中の壊れリンクが
    // 検査をすり抜ける。fmt が未参照定義を温存する以上、検査も同じ AST を見る
    let options = comrak_options_keep_footnotes(opts);
    let root = parse_document(&arena, source, &options);

    let mut refs = Vec::new();
    for node in root.descendants() {
        let data = node.data.borrow();
        let (url, is_image) = match &data.value {
            NodeValue::Link(link) => (link.url.clone(), false),
            NodeValue::Image(link) => (link.url.clone(), true),
            _ => continue,
        };
        refs.push(LinkRef {
            url,
            is_image,
            span: span_of(&data.sourcepos),
        });
    }
    refs
}

/// 本文のテキストノードを span 付きで列挙する（用語 lint 用）。
/// コードブロック・インラインコード・HTML・数式・リンク URL は Text ノードに
/// ならないため対象外になる（見出し・リンクラベル・強調中のテキストは含む）
pub(crate) fn extract_text_spans(
    source: &str,
    opts: &MarkdownOptions,
) -> Vec<(String, SourceSpan)> {
    let arena = Arena::new();
    let root = parse_document(&arena, source, &comrak_options(opts));
    let mut out = Vec::new();
    for node in root.descendants() {
        let data = node.data.borrow();
        if let NodeValue::Text(text) = &data.value {
            out.push((text.to_string(), span_of(&data.sourcepos)));
        }
    }
    out
}

/// 抑制コメント 1 件（分類とソース上の位置）
pub(crate) struct SuppressComment {
    pub kind: suppress_comment::SuppressCommentKind,
    /// コメント自身の span（invalid / unused の報告位置）
    pub span: SourceSpan,
}

/// 行単位の抑制コメント（`<!-- yuzu-lint-… -->`）を文書順に列挙する（`suppress.rs` 用）。
///
/// `comrak_options` を使う（keep_footnotes 版ではない）: 照合相手の診断を作る
/// [`extract_text_spans`] / [`extract_fence_meta`] と同じ AST 族に揃える。
/// keep_footnotes が救う「未参照の脚注定義」の中は lint 診断が出ない場所なので、
/// 抑制コメントを拾っても対象がない。
///
/// コードブロック・インラインコード内は comrak が HtmlBlock にしないため
/// 構造的に対象外（docs に記法例をフェンスで安全に書ける根拠）
pub(crate) fn extract_suppress_comments(
    source: &str,
    opts: &MarkdownOptions,
) -> Vec<SuppressComment> {
    use suppress_comment::SuppressCommentKind;

    let arena = Arena::new();
    let root = parse_document(&arena, source, &comrak_options(opts));
    let mut out = Vec::new();
    for node in root.descendants() {
        let data = node.data.borrow();
        match &data.value {
            NodeValue::HtmlBlock(nhb) => {
                let first_line = nhb.literal.lines().next().unwrap_or("");
                let Some(kind) = suppress_comment::classify_comment_line(first_line) else {
                    continue;
                };
                let mut span = span_of(&data.sourcepos);
                // 閉じ忘れは HtmlBlock が文書末尾まで届くので、報告位置を開始行に絞る
                if kind == SuppressCommentKind::Unclosed {
                    span.end_line = span.start_line;
                    span.end_col = (span.start_col + first_line.chars().count()).max(1);
                }
                out.push(SuppressComment { kind, span });
            }
            // 段落中のインラインコメント: `yuzu-lint-` 接頭なら「単独行でない」誤用
            NodeValue::HtmlInline(literal)
                if suppress_comment::classify_comment_line(literal).is_some() =>
            {
                out.push(SuppressComment {
                    kind: SuppressCommentKind::NotStandalone,
                    span: span_of(&data.sourcepos),
                });
            }
            _ => {}
        }
    }
    out
}

/// frontmatter の生テキスト（`---` 区切り行込み）とソース上の位置を返す。
/// frontmatter がなければ None（lint の未知キー検出用）
pub(crate) fn frontmatter_raw(
    source: &str,
    opts: &MarkdownOptions,
) -> Option<(String, SourceSpan)> {
    let arena = Arena::new();
    let options = comrak_options(opts);
    let root = parse_document(&arena, source, &options);

    let first = root.first_child()?;
    let data = first.data.borrow();
    match &data.value {
        NodeValue::FrontMatter(raw) => Some((raw.clone(), span_of(&data.sourcepos))),
        _ => None,
    }
}

/// 全文を整形した Markdown を返す（`yuzu fmt` 用）。
///
/// 本文は [`normalize_markdown`] と同じ `format_commonmark` の正規形。
/// frontmatter は YAML を再シリアライズせず**生テキストをバイト温存**して
/// 再結合する（コメント・キー順・クォートを壊さない）。
/// 末尾改行は常に 1 個、frontmatter と本文の間は空行 1 つに正規化する
pub(crate) fn format_document(source: &str, opts: &MarkdownOptions) -> Result<String, CoreError> {
    let arena = Arena::new();
    // 脚注定義の位置・未参照定義を温存する（fmt は書き手の構成を動かさない）
    let options = comrak_options_keep_footnotes(opts);
    let root = parse_document(&arena, source, &options);

    // frontmatter の生テキスト（区切り行込み）を退避して detach
    // （format_commonmark が生テキストごと再出力してしまうため。normalize と同じ）
    let mut fm_raw: Option<String> = None;
    if let Some(first) = root.first_child() {
        if let NodeValue::FrontMatter(raw) = &first.data.borrow().value {
            fm_raw = Some(raw.clone());
        }
        if fm_raw.is_some() {
            first.detach();
        }
    }

    let mut body = String::new();
    format_commonmark(root, &options, &mut body)?;
    let body = restore_yuzu_syntax(body);
    let body = body.trim_end();

    Ok(match (fm_raw, body.is_empty()) {
        (Some(raw), true) => format!("{}\n", raw.trim_end()),
        (Some(raw), false) => format!("{}\n\n{body}\n", raw.trim_end()),
        (None, true) => String::new(),
        (None, false) => format!("{body}\n"),
    })
}

/// 本文を h2/h3 見出し境界で分割したプレーンテキストセクションを返す（検索インデックス用）。
///
/// - 先頭は常にリード文セクション（anchor/heading = None。本文が無くても返す）
/// - h4〜h6 と h1 は境界にせず、見出しテキストを現セクションの本文に含める
/// - 収集: `Text` / インライン `Code`（API 名検索のため含める）。
///   `SoftBreak` / `LineBreak` は空白、ブロック要素の末尾で改行 1 つ
/// - 除外: frontmatter・生 HTML。**フェンスコードブロック**は既定で除外だが
///   `index_code = true`（`search.indexCode`）のとき本文を含める。ただし
///   インデントコードブロック（非フェンス）と、特別レンダリングされる言語
///   （[`crate::is_special_render_lang`]。無効化されてプレーン表示なら索引対象）は除外
///
/// ⚠️ アンカー同期: [`extract_meta`]・HTML 化と同じく Anchorizer を
/// **全見出し（h1〜h6）文書順**で回す。境界にしない見出しも必ず anchorize する。
/// keep_footnotes 版オプションは使わない（採番が render とずれるため）
pub(crate) fn extract_plain_sections(
    source: &str,
    opts: &MarkdownOptions,
    index_code: bool,
    project_root: Option<&Path>,
) -> Result<Vec<PlainSection>, CoreError> {
    let arena = Arena::new();
    let options = comrak_options(opts);
    let root = parse_document(&arena, source, &options);

    let mut anchorizer = Anchorizer::new();
    let mut sections = vec![PlainSection {
        anchor: None,
        heading: None,
        body: String::new(),
    }];
    collect_sections(
        root,
        &mut anchorizer,
        &mut sections,
        opts,
        index_code,
        project_root,
    );
    for section in &mut sections {
        section.body = section.body.trim().to_string();
    }
    Ok(sections)
}

/// 用語の初出を `<abbr>` 化する対象を集める（適用は呼び出し側）。
///
/// `descendants()` ではなく `collect_sections` と同じ自前再帰にするのは、
/// **種別ごとに配下ごと打ち切る**（early return）ためで、comrak の AST では
/// 親を辿れないので文脈による除外はこの形でしか書けない。
///
/// 除外の理由は種別ごとに違う:
/// - `Image` — **必須**。alt テキストは comrak が「HTML を素通しできない文脈」として
///   描くため、`HtmlInline` のリテラルがエスケープされて `alt="&lt;abbr …"` になる
/// - `Heading` / `Link` — 方針。初出は散文で消費させたい（見出しやリンク文字列で
///   使い切ると本文中の説明が出ない）し、リンク文字列は著者の指定を尊重する
/// - `Code` / `Math` / `CodeBlock` / `HtmlBlock` / `HtmlInline` は**そもそも `Text` に
///   ならない**ので、子へ降りても対象にならない（インラインコード・数式・生 HTML）
fn collect_abbr<'a>(
    node: &'a AstNode<'a>,
    matcher: &glossary::Matcher<'_>,
    used: &mut std::collections::HashSet<String>,
    out: &mut Vec<(&'a AstNode<'a>, Vec<glossary::Piece>)>,
) {
    {
        let data = node.data.borrow();
        match &data.value {
            NodeValue::Heading(_)
            | NodeValue::Image(_)
            | NodeValue::Link(_)
            | NodeValue::FrontMatter(_) => return,
            NodeValue::Text(literal) => {
                if let Some(pieces) = matcher.split(literal, used) {
                    out.push((node, pieces));
                }
                return;
            }
            _ => {}
        }
    }
    for child in node.children() {
        collect_abbr(child, matcher, used, out);
    }
}

fn collect_sections<'a>(
    node: &'a AstNode<'a>,
    anchorizer: &mut Anchorizer,
    sections: &mut Vec<PlainSection>,
    opts: &MarkdownOptions,
    index_code: bool,
    project_root: Option<&Path>,
) {
    {
        let data = node.data.borrow();
        match &data.value {
            NodeValue::FrontMatter(_) | NodeValue::HtmlBlock(_) | NodeValue::HtmlInline(_) => {
                return;
            }
            NodeValue::CodeBlock(cb) => {
                let (lang, meta) = parse_fence_info(&cb.info);
                // Markdown 断片（```include）は散文なので indexCode と無関係に
                // 常に索引する。記法を落としたプレーンテキストを現セクションへ
                // 足す（見出し境界は作らない = 断片に見出しは来ない契約）。
                // 読めない場合は黙って諦める（既存の include と同じ方針）
                if lang == Some(fragment::FRAGMENT_LANG) {
                    if let (Some(spec), Some(root)) = (&meta.include, project_root) {
                        if let Ok(text) = crate::include::resolve_include(root, spec) {
                            sections
                                .last_mut()
                                .unwrap()
                                .body
                                .push_str(&fragment::collect_plain_text(&text, opts));
                        }
                    }
                    return;
                }
                // 既定は除外。`index_code` の opt-in 時のみ、フェンスコードブロックに限り
                // 本文を含める（インデントコードは公開ドキュメントの「フェンス」に合わせ除外）。
                // 特別レンダリングされる言語（図・仕様・数式ソース）は検索ノイズなので除外
                // — ただし機能が無効でプレーンコード表示になる場合は見えるまま索引する
                let lang = lang.unwrap_or("");
                if !index_code || !cb.fenced || crate::is_special_render_lang(lang, opts) {
                    return;
                }
                // コンテンツインクルードは literal が空なので、参照先を読んで索引する
                // （表示されている内容を検索できる、を保つ。読めない場合は黙って諦め、
                // エラーの可視化は描画のエラーボックスと `yuzu check` の責務）
                match (&meta.include, project_root) {
                    (Some(spec), Some(root)) => {
                        if let Ok(text) = crate::include::resolve_include(root, spec) {
                            sections.last_mut().unwrap().body.push_str(&text);
                        }
                    }
                    _ => sections.last_mut().unwrap().body.push_str(&cb.literal),
                }
                // return せず末尾のブロック改行へ流し、トークンの文脈を切る
            }
            NodeValue::Heading(heading) => {
                let text = collect_text(node);
                let id = anchorizer.anchorize(&text);
                if heading.level == 2 || heading.level == 3 {
                    // 境界: 新しいセクションを開始（自見出しは body に含めず、
                    // builder が heading フィールドへ重み付きで別計上する）
                    sections.push(PlainSection {
                        anchor: Some(id),
                        heading: Some(text),
                        body: String::new(),
                    });
                } else {
                    // h1・h4〜h6 は境界にしない（テキストは検索対象として本文に残す）
                    let body = &mut sections.last_mut().expect("先頭セクションが常にある").body;
                    body.push_str(&text);
                    body.push('\n');
                }
                return; // 見出し配下は collect_text で回収済み
            }
            NodeValue::Text(literal) => sections.last_mut().unwrap().body.push_str(literal),
            NodeValue::Code(code) => sections.last_mut().unwrap().body.push_str(&code.literal),
            NodeValue::LineBreak | NodeValue::SoftBreak => {
                sections.last_mut().unwrap().body.push(' ')
            }
            _ => {}
        }
    }

    for child in node.children() {
        collect_sections(child, anchorizer, sections, opts, index_code, project_root);
    }

    // 段落・リスト項目等の区切りで改行を入れる（トークナイズの文脈を切る）
    if node.data.borrow().value.block() {
        let body = &mut sections.last_mut().unwrap().body;
        if !body.is_empty() && !body.ends_with('\n') {
            body.push('\n');
        }
    }
}

/// 見出しノード配下のプレーンテキストを収集する。
/// comrak の header_ids 拡張と同じ規則（Text/Code はリテラル、改行は空白）に合わせる
fn collect_text<'a>(node: &'a AstNode<'a>) -> String {
    let mut out = String::new();
    collect_text_into(node, &mut out);
    out
}

fn collect_text_into<'a>(node: &'a AstNode<'a>, out: &mut String) {
    match &node.data.borrow().value {
        NodeValue::Text(literal) => out.push_str(literal),
        NodeValue::Code(code) => out.push_str(&code.literal),
        // comrak の header_ids（Anchorizer）は見出し内数式の literal を採番に含める。
        // ここで落とすと TOC・リンク検査のアンカーが本文とずれる
        NodeValue::Math(math) => out.push_str(&math.literal),
        NodeValue::LineBreak | NodeValue::SoftBreak => out.push(' '),
        _ => {
            for child in node.children() {
                collect_text_into(child, out);
            }
        }
    }
}

/// lint（`code-block-meta`）用: fenced コードブロックの情報文字列と位置・コード行数
pub(crate) struct FenceMeta {
    pub info: String,
    /// ブロック全体の位置（診断表示には開始行を使う）
    pub span: SourceSpan,
    /// コード本文の行数（行ハイライトの範囲外検査用）
    pub code_lines: usize,
    /// `tab=` が実際にグループを作れるか（隣接する兄弟にも `tab=` がある）。
    /// false のまま `tab=` が書かれていると「指定したのに効かない」ので lint が警告する
    pub tab_grouped: bool,
}

/// フェンスコードブロック 1 つを本文つきで返す（core の外で本文を解釈する検査用）
pub struct FenceBlock {
    /// 情報文字列の先頭トークン（` ```openapi ` なら `Some("openapi")`）
    pub lang: Option<String>,
    /// フェンス本文（改行込み）
    pub body: String,
    /// ブロック全体の位置（診断表示には開始行を使う）
    pub span: SourceSpan,
}

/// fenced コードブロックを本文つきで列挙する。comrak を触るのはこのモジュールだけ
/// なので、`yuzu check` の apispec 検証のように本文を crate 外で解釈する経路は
/// これを使う（[`extract_fence_meta`] は情報文字列と行数しか持たない）
pub fn extract_fence_blocks(source: &str, opts: &MarkdownOptions) -> Vec<FenceBlock> {
    let arena = Arena::new();
    let options = comrak_options(opts);
    let root = parse_document(&arena, source, &options);
    let mut out = Vec::new();
    for node in root.descendants() {
        let data = node.data.borrow();
        if let NodeValue::CodeBlock(cb) = &data.value {
            if !cb.fenced {
                continue;
            }
            out.push(FenceBlock {
                lang: cb
                    .info
                    .split_whitespace()
                    .next()
                    .filter(|s| !s.is_empty())
                    .map(str::to_string),
                body: cb.literal.clone(),
                span: span_of(&data.sourcepos),
            });
        }
    }
    out
}

/// fenced コードブロックだけを列挙する（インデントコードは対象外）
pub(crate) fn extract_fence_meta(source: &str, opts: &MarkdownOptions) -> Vec<FenceMeta> {
    let arena = Arena::new();
    let options = comrak_options(opts);
    let root = parse_document(&arena, source, &options);
    let mut out = Vec::new();
    for node in root.descendants() {
        let data = node.data.borrow();
        if let NodeValue::CodeBlock(cb) = &data.value {
            if !cb.fenced {
                continue;
            }
            let code_lines = cb.literal.lines().count();
            let span = span_of(&data.sourcepos);
            let info = cb.info.clone();
            drop(data);
            out.push(FenceMeta {
                info,
                span,
                code_lines,
                tab_grouped: tabs::has_tab_neighbor(node, opts),
            });
        }
    }
    out
}

fn span_of(sourcepos: &comrak::nodes::Sourcepos) -> SourceSpan {
    SourceSpan {
        start_line: sourcepos.start.line,
        start_col: sourcepos.start.column,
        end_line: sourcepos.end.line,
        end_col: sourcepos.end.column,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// インラインノード（リンク）の sourcepos が行・列とも正確なことの実測
    /// （linkcheck の診断位置の前提。ずれるようなら表示を行番号のみに落とす）
    #[test]
    fn リンクの_sourcepos_は行と列を正しく指す() {
        let source = "# 見出し\n\n本文 [リンク](target.md) と ![画像](img.png)。\n\n- 項目の [中のリンク](other.md#frag)\n";
        let refs = extract_link_refs(source, &MarkdownOptions::default());
        assert_eq!(refs.len(), 3);

        assert_eq!(refs[0].url, "target.md");
        assert!(!refs[0].is_image);
        assert_eq!(refs[0].span.start_line, 3);
        // 「本文 」= 本文(6 バイト) + 空白(1) の次 = 8 バイト目（col は 1 始まりバイト位置）
        assert_eq!(refs[0].span.start_col, 8);

        assert_eq!(refs[1].url, "img.png");
        assert!(refs[1].is_image);
        assert_eq!(refs[1].span.start_line, 3);

        assert_eq!(refs[2].url, "other.md#frag");
        assert_eq!(refs[2].span.start_line, 5);
    }

    /// フェンス情報文字列の表示メタ（title / 行ハイライト / 行番号）は
    /// fmt の正規化で逐語温存され、冪等であること（Phase 39 の不変条件）
    #[test]
    fn fmt_はフェンスの表示メタを温存し冪等() {
        let source = "# 見出し\n\n```rust title=\"src/main.rs\" {2,4-6} showLineNumbers\nfn main() {}\n```\n";
        let opts = MarkdownOptions::default();
        let once = format_document(source, &opts).unwrap();
        assert!(
            once.contains("```rust title=\"src/main.rs\" {2,4-6} showLineNumbers\n"),
            "情報文字列が逐語温存される:\n{once}"
        );
        let twice = format_document(&once, &opts).unwrap();
        assert_eq!(once, twice, "冪等");
    }
}
