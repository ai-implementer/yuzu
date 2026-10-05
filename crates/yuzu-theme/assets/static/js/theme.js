// ダークモード切替（theme.dark = "toggle" のときだけボタンがある）。
// 選択は localStorage("yuzu-theme") に保存する。初期適用は base.jinja の head 内
// インラインスクリプト（FOUC 回避）が行う。選択が無いあいだ data-theme は付かず、
// CSS の prefers-color-scheme が OS の設定に従う
(function () {
  var button = document.getElementById("theme-toggle");
  if (!button) return;
  var osDark = window.matchMedia("(prefers-color-scheme: dark)");
  button.addEventListener("click", function () {
    var html = document.documentElement;
    // 今見えている配色（明示の選択が無ければ OS の設定）の反対にする
    var current = html.dataset.theme || (osDark.matches ? "dark" : "light");
    var next = current === "dark" ? "light" : "dark";
    html.dataset.theme = next;
    try {
      localStorage.setItem("yuzu-theme", next);
    } catch (e) {
      /* プライベートブラウジング等では保存しない */
    }
  });
})();
