// yuzu dev 用の WebSocket ライブリロード。/__livereload（base に依らずルート固定）
// に接続し、"reload" 受信でページを再読込する。切断時は 1 秒間隔で再接続し、
// 再接続に成功したら（＝サーバ再起動後）最新を取り直すためリロードする。
(function () {
  var RETRY_MS = 1000;
  var wasConnected = false;

  function connect() {
    var proto = location.protocol === "https:" ? "wss://" : "ws://";
    var ws = new WebSocket(proto + location.host + "/__livereload");

    ws.onopen = function () {
      if (wasConnected) {
        // サーバ再起動をまたいだ編集を取りこぼさない。
        // 初回接続では reload しない（リロードループ防止）
        location.reload();
        return;
      }
      wasConnected = true;
    };
    ws.onmessage = function (ev) {
      if (ev.data === "reload") location.reload();
    };
    ws.onclose = function () {
      setTimeout(connect, RETRY_MS);
    };
    // onerror は直後に onclose が来るので何もしない
  }

  connect();
})();
