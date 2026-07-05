// 狭幅でのサイドバーナビ開閉（プログレッシブエンハンスメント）。
// JS 無効時はボタンが hidden のままで、ナビは従来どおり常時展開される
(function () {
  var toggle = document.getElementById("nav-toggle");
  var sidebar = document.getElementById("site-sidebar");
  if (!toggle || !sidebar) return;

  // JS が動いたときだけ「閉じた状態を既定」にする
  toggle.hidden = false;
  document.body.classList.add("has-nav-js");

  toggle.addEventListener("click", function () {
    var open = document.body.classList.toggle("nav-open");
    toggle.setAttribute("aria-expanded", open ? "true" : "false");
  });
})();
