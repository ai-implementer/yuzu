// コードブロックのコピーボタン（プログレッシブエンハンスメント）。
// JS 無効・Clipboard API 非対応（非 https 配信等）では何も挿入せず表示は従来どおり。
// mermaid（pre.mermaid / figure.mermaid-ssr）は pre > code を持たないため対象外
(function () {
  if (!(navigator.clipboard && navigator.clipboard.writeText)) return;

  var codes = document.querySelectorAll(".markdown-body pre > code");
  if (codes.length === 0) return;
  // CSS 側の言語ラベル回避（ボタン幅ぶんの margin）を発動させる
  document.body.classList.add("has-copy-js");

  for (var i = 0; i < codes.length; i++) {
    attach(codes[i]);
  }

  function attach(code) {
    var pre = code.parentNode;
    if (pre.classList.contains("mermaid")) return; // 防御（現状 pre.mermaid に code は無い）

    var button = document.createElement("button");
    button.type = "button";
    button.className = "copy-button";
    button.textContent = "コピー";
    // テキスト変化（コピーしました）をスクリーンリーダーへ通知する
    button.setAttribute("aria-live", "polite");

    var timer = null;
    button.addEventListener("click", function () {
      navigator.clipboard.writeText(code.textContent).then(
        function () {
          feedback("コピーしました");
        },
        function () {
          feedback("コピーできません");
        }
      );
    });

    function feedback(message) {
      button.textContent = message;
      button.classList.add("copied");
      if (timer) clearTimeout(timer);
      timer = setTimeout(function () {
        button.textContent = "コピー";
        button.classList.remove("copied");
        timer = null;
      }, 2000);
    }

    pre.appendChild(button);
  }
})();
