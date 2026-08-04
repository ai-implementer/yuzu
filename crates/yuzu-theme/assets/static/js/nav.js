// 狭幅でのサイドバーナビ開閉（プログレッシブエンハンスメント）。
// JS 無効時はボタンが hidden のままで、ナビは従来どおり常時展開される。
// 開いた後の「閉じる導線」も持つ: Esc・ナビリンクのクリック・外側クリック
// （nav-open は狭幅のトグル経由でしか立たないため、広幅では実質何もしない）
(function () {
  var toggle = document.getElementById("nav-toggle");
  var sidebar = document.getElementById("site-sidebar");
  if (!toggle || !sidebar) return;

  // JS が動いたときだけ「閉じた状態を既定」にする
  toggle.hidden = false;
  document.body.classList.add("has-nav-js");

  function isOpen() {
    return document.body.classList.contains("nav-open");
  }

  function setOpen(open) {
    document.body.classList.toggle("nav-open", open);
    toggle.setAttribute("aria-expanded", open ? "true" : "false");
  }

  toggle.addEventListener("click", function () {
    setOpen(!isOpen());
  });

  // Esc で閉じ、開閉ボタンへフォーカスを返す（キーボード操作の迷子防止）
  document.addEventListener("keydown", function (ev) {
    if (ev.key !== "Escape" || !isOpen()) return;
    setOpen(false);
    toggle.focus();
  });

  // ナビ内のリンクをクリックしたら閉じる（開きっぱなしの本文押し下げを残さない）。
  // summary（セクション開閉）のクリックでは閉じない = path に <a> が居ない。
  // ⚠️ 判定は composedPath（closest は再描画で外れた要素に対して誤判定する）
  sidebar.addEventListener("click", function (ev) {
    if (!isOpen()) return;
    var path = ev.composedPath ? ev.composedPath() : [];
    for (var i = 0; i < path.length; i++) {
      if (path[i] === sidebar) break;
      if (path[i].tagName === "A") {
        setOpen(false);
        return;
      }
    }
  });

  // 外側クリック（本文側のタップ）で閉じる。開閉ボタン自身のクリックは
  // path に含まれるため、開いた直後に外側判定で閉じ戻る誤判定は起きない
  document.addEventListener("click", function (ev) {
    if (!isOpen()) return;
    var path = ev.composedPath ? ev.composedPath() : [];
    if (path.indexOf(sidebar) === -1 && path.indexOf(toggle) === -1) {
      setOpen(false);
    }
  });
})();
