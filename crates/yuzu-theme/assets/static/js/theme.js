// ダークモード切替。選択は localStorage("yuzu-theme") に保存する。
// 初期適用は base.jinja の head 内インラインスクリプト（FOUC 回避）が行う。
(function () {
  var button = document.getElementById("theme-toggle");
  if (!button) return;
  button.addEventListener("click", function () {
    var html = document.documentElement;
    var next = html.dataset.theme === "dark" ? "light" : "dark";
    html.dataset.theme = next;
    try {
      localStorage.setItem("yuzu-theme", next);
    } catch (e) {
      /* プライベートブラウジング等では保存しない */
    }
  });
})();
