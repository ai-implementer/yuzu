// build --watch 用の簡易オートリフレッシュ（WebSocket は使わない。Phase 2 で WS 化）。
// dist/__yuzu/build_id を 1 秒間隔でポーリングし、変化したらリロードする。
(function () {
  var base = (document.currentScript && document.currentScript.dataset.base) || "/";
  var current = null;
  setInterval(function () {
    fetch(base + "__yuzu/build_id", { cache: "no-store" })
      .then(function (res) {
        return res.ok ? res.text() : null;
      })
      .then(function (id) {
        if (id === null) return;
        if (current === null) {
          current = id;
        } else if (id !== current) {
          location.reload();
        }
      })
      .catch(function () {
        /* 再ビルド中の一時的な失敗は無視 */
      });
  }, 1000);
})();
