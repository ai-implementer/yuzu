// 配色の切替（theme.dark = "toggle" のときだけボタンがある）。押すたびに
// 「OS の設定」→ 今の見た目の反対 → もう一方 →「OS の設定」の順に変わる（Phase 90。以前は
// ライトとダークの行き来だけで、一度選ぶと OS の設定に戻れなかった。最初の 1 回は必ず見た目が
// 変わるよう、OS の設定の次は今の見た目の反対にする）。
// 選択は localStorage("yuzu-theme") に保存する（OS の設定に戻すときは消す）。初期適用は
// base.jinja の head 内インラインスクリプト（FOUC 回避）が行う。選択が無いあいだ data-theme は
// 付かず、CSS の prefers-color-scheme が OS の設定に従う。今の状態はボタンの aria-label と
// title に出す
(function () {
  var button = document.getElementById("theme-toggle");
  if (!button) return;
  var osDark = window.matchMedia("(prefers-color-scheme: dark)");
  var labels = { os: "OS の設定", light: "ライト", dark: "ダーク" };
  var icons = { os: "◐", light: "☀", dark: "☾" };

  function current() {
    var theme = document.documentElement.dataset.theme;
    return theme === "light" || theme === "dark" ? theme : "os";
  }

  function next(state) {
    if (state === "os") return osDark.matches ? "light" : "dark";
    // OS の設定の次に選んだほう（今の見た目の反対）→ もう一方 → OS の設定
    var first = osDark.matches ? "light" : "dark";
    return state === first ? (first === "light" ? "dark" : "light") : "os";
  }

  function render() {
    var state = current();
    var label = "配色: " + labels[state] + "（次: " + labels[next(state)] + "）";
    button.setAttribute("aria-label", label);
    button.title = label;
    button.textContent = icons[state];
  }

  button.addEventListener("click", function () {
    var html = document.documentElement;
    var state = next(current());
    if (state === "os") {
      delete html.dataset.theme;
    } else {
      html.dataset.theme = state;
    }
    try {
      if (state === "os") {
        localStorage.removeItem("yuzu-theme");
      } else {
        localStorage.setItem("yuzu-theme", state);
      }
    } catch (e) {
      /* プライベートブラウジング等では保存しない */
    }
    render();
  });
  // OS の設定が変わると次の状態も変わるので、ラベルの「次:」を合わせる
  osDark.addEventListener("change", render);
  render();
})();
