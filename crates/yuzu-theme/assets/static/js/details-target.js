// 折りたたみ（details）の中へアンカーで飛んだとき、祖先を自動で開く。
// 検索結果・目次・図表の相互参照からのジャンプで「閉じたままで中身が見えない」
// のを防ぐ。あわせて印刷時（beforeprint）に閉じた details を全開し、
// 印刷後（afterprint）に閉じ直す（PDF に折りたたみの中身を漏れなく載せるため）。
//
// プログレッシブエンハンスメント: JS 無しでも中身は HTML に含まれていて
// クリックで開ける（折りたたみ自体は details のネイティブ動作で JS 不要）。
// ページ内検索（Cmd/Ctrl+F）での自動展開はブラウザ側の対応に任せる
// （Chrome は details を自動で開く。ここで hidden=until-found を足しても
// 閉じた details の中身は details 自身が隠すため効果がない）。
(function () {
  function revealHash() {
    const id = decodeURIComponent(location.hash.slice(1));
    if (!id) return;
    const target = document.getElementById(id);
    if (!target) return;

    let opened = false;
    for (let node = target; node; node = node.parentElement) {
      if (node.tagName === "DETAILS" && !node.open) {
        node.open = true;
        opened = true;
      }
    }
    // 開いた分だけレイアウトが下へずれるので、ブラウザ既定のスクロールを補正する
    if (opened) target.scrollIntoView();
  }

  revealHash(); // 読み込み時のハッシュ
  window.addEventListener("hashchange", revealHash); // 同一ページ内の移動

  // 印刷: 閉じた折りたたみを beforeprint で全開し、afterprint で閉じ直す。
  // CSS では details を開けない（open は HTML 属性で、閉じた中身はレイアウト外）
  // ため JS で行う。元から開いているものには触らず、ここで開いたものだけを
  // 記録して戻す。記録のクリアは afterprint 側だけで行う — beforeprint が
  // 対にならず連続発火しても（プレビューの再表示等）復元対象を失わない
  //（2 回目は該当 details が既に open なので querySelectorAll が空になる）
  let printOpened = [];
  window.addEventListener("beforeprint", function () {
    for (const node of document.querySelectorAll("details:not([open])")) {
      node.open = true;
      printOpened.push(node);
    }
  });
  window.addEventListener("afterprint", function () {
    for (const node of printOpened) node.open = false;
    printOpened = [];
  });
})();
