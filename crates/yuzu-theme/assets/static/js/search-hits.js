// 検索ヒットの描画部品。ドロップダウン（search-ui.js）と検索結果ページ
// （search-page.js）の 1 実装共有 — 同じ規則を 2 箇所で解釈すると必ずズレる。
// DOM の組み立てだけを担当し、role / id / イベントの付与は呼び出し側の責務
// （listbox に入れるのはドロップダウンだけ）

// 抜粋の最大文字数
export const EXCERPT_CHARS = 160;

// wasm の excerpt（エンジンと同一の分かち書き・正規化）で <mark> 断片列を作る。
// XSS 安全: 文字列は必ず createTextNode / textContent 経由で DOM 化する。
// maxChars 既定 10000 = タイトル用の実質切り詰めなし（一致がなければ原文のまま）
export function markSegments(client, text, query, maxChars = 10000) {
  const segments = client.excerpt(text, query, maxChars);
  return segments.map((seg) => {
    if (!seg.mark) return document.createTextNode(seg.text);
    const mark = document.createElement("mark");
    mark.textContent = seg.text;
    return mark;
  });
}

// ヒット 1 件 = <a class="search-hit">（タイトル ＋ › 見出し ＋ 抜粋）。
// セクション doc は見出しアンカーへ直接ジャンプする
export function hitLink(client, fragment, query, base) {
  const a = document.createElement("a");
  a.className = "search-hit";
  a.href = base + fragment.url + (fragment.anchor ? "#" + fragment.anchor : "");
  const title = document.createElement("div");
  title.className = "search-hit-title";
  title.append(...markSegments(client, fragment.title, query));
  if (fragment.heading) {
    const crumb = document.createElement("span");
    crumb.className = "search-hit-crumb";
    crumb.append(" › ", ...markSegments(client, fragment.heading, query));
    title.append(crumb);
  }
  const excerpt = document.createElement("div");
  excerpt.className = "search-hit-excerpt";
  excerpt.append(...markSegments(client, fragment.text, query, EXCERPT_CHARS));
  a.append(title, excerpt);
  return a;
}

// フレーズ検索の発見用ヒント行（引用符を既に使っているクエリでは null）
export function hintRow(query) {
  if (/["＂“”]/.test(query)) return null;
  const hint = document.createElement("div");
  hint.className = "search-hint";
  hint.textContent = '"..." で囲むと完全一致（フレーズ）検索';
  return hint;
}
