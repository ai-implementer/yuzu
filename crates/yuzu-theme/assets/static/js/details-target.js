// 折りたたみ（details）の中へアンカーで飛んだとき、祖先を自動で開く。
// 検索結果・目次・図表の相互参照からのジャンプで「閉じたままで中身が見えない」
// のを防ぐ。
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
})();
