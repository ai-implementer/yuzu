// KaTeX によるクライアント数式描画（プログレッシブエンハンスメント）。
// KaTeX 未読込（vendor 未取得等）・描画失敗時は原文（TeX ソース）表示のまま。
// ```math の <pre> はコピーボタンごと描画結果に置換される（copy-button.js の
// ボタン挿入 → 本スクリプトの置換、が同期実行の同一タスク内で完結する）
(function () {
  if (!window.katex) return;

  var nodes = document.querySelectorAll(".markdown-body [data-math-style]");
  for (var i = 0; i < nodes.length; i++) {
    render(nodes[i]);
  }

  function render(el) {
    var display = el.getAttribute("data-math-style") === "display";
    var target = document.createElement(display ? "div" : "span");
    target.className = display ? "math math-display" : "math math-inline";
    try {
      // throwOnError: false → TeX 構文エラーは KaTeX が原文を赤字表示する
      window.katex.render(el.textContent, target, {
        displayMode: display,
        throwOnError: false,
      });
    } catch (e) {
      return; // 想定外の失敗は原文表示のまま（ページを壊さない）
    }
    // ```math（pre > code）は pre ごと置換（pre の枠・背景・コピーボタンを残さない）。
    // span / code のインライン系はその要素だけ置換
    var victim =
      el.tagName === "CODE" && el.parentNode && el.parentNode.tagName === "PRE"
        ? el.parentNode
        : el;
    victim.parentNode.replaceChild(target, victim);
  }
})();
