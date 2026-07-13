// 「Markdown をコピー」ボタン（プログレッシブエンハンスメント）。
// article[data-md-url] からページ単位 Markdown を fetch してクリップボードへ。
// JS 無効・clipboard 非対応（非 HTTPS 等）ではボタン自体を出さない。
(function () {
  var article = document.querySelector("article[data-md-url]");
  if (!article || !navigator.clipboard || !window.fetch) return;
  var mdUrl = article.dataset.mdUrl;

  var actions = document.createElement("div");
  actions.className = "page-actions";

  var button = document.createElement("button");
  button.type = "button";
  button.className = "page-copy";
  var defaultLabel = "Markdown をコピー";
  button.textContent = defaultLabel;
  button.addEventListener("click", function () {
    fetch(mdUrl)
      .then(function (res) {
        if (!res.ok) throw new Error("HTTP " + res.status);
        return res.text();
      })
      .then(function (text) {
        return navigator.clipboard.writeText(text);
      })
      .then(function () {
        button.textContent = "コピーしました";
        button.classList.add("copied");
      })
      .catch(function () {
        button.textContent = "コピーに失敗しました";
      })
      .then(function () {
        setTimeout(function () {
          button.textContent = defaultLabel;
          button.classList.remove("copied");
        }, 2000);
      });
  });

  var open = document.createElement("a");
  open.className = "page-md-link";
  open.href = mdUrl;
  open.textContent = ".md を開く";

  actions.appendChild(button);
  actions.appendChild(open);
  article.parentNode.insertBefore(actions, article);
})();
