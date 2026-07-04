// Mermaid のクライアント描画（v0.1 凍結方針）。
// 既知の制限: ダークモード切替後の再描画はしない（リロードで反映）。
(function () {
  if (!window.mermaid) return;
  var dark = document.documentElement.dataset.theme === "dark";
  window.mermaid.initialize({
    startOnLoad: false,
    theme: dark ? "dark" : "default",
  });
  window.mermaid.run();
})();
