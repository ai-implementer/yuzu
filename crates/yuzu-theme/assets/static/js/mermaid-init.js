// Mermaid のクライアント描画。
// ダークモード切替（html[data-theme] の変化）を監視して同じ図を再描画する。
// mermaid.run() は描画時に要素の中身を SVG へ差し替えるため、
// 初回に元ソースを退避しておき、再描画前に戻す。
(function () {
  if (!window.mermaid) return;
  var blocks = Array.prototype.slice.call(document.querySelectorAll("pre.mermaid"));
  if (blocks.length === 0) return;
  blocks.forEach(function (el) {
    el.dataset.mermaidSource = el.textContent;
  });

  function render() {
    var dark = document.documentElement.dataset.theme === "dark";
    window.mermaid.initialize({
      startOnLoad: false,
      theme: dark ? "dark" : "default",
    });
    window.mermaid.run();
  }

  render();

  // theme.js（ボタン）以外の切替経路にも追従できるよう属性変化を監視する
  new MutationObserver(function () {
    blocks.forEach(function (el) {
      el.removeAttribute("data-processed");
      el.textContent = el.dataset.mermaidSource;
    });
    render();
  }).observe(document.documentElement, {
    attributes: true,
    attributeFilter: ["data-theme"],
  });
})();
