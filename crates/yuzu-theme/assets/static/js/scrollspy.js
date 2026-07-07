// ページ内 TOC のスクロールスパイ（プログレッシブエンハンスメント）。
// 「現在のセクション」＝ 基準線（html の scroll-padding-top の位置）を上に越えた最後の見出し。
// 見出し位置はキャッシュしない（mermaid のクライアント描画や画像ロードで
// レイアウト高が変わるため、毎回 getBoundingClientRect で判定する）
(function () {
  // 右カラム .toc とモバイル .toc-mobile の両方に同じ id へのリンクがある。
  // id は日本語を含むため getAttribute("href")（生 UTF-8）＋ getElementById で解決する
  //（a.hash はパーセントエンコード済み、querySelector("#…") は構文エラーになるため不可）
  var links = document.querySelectorAll(".toc a[href^='#'], .toc-mobile a[href^='#']");
  if (links.length === 0) return;

  var entries = []; // 文書順（TOC の出力順）
  var byId = {};
  for (var i = 0; i < links.length; i++) {
    var id = links[i].getAttribute("href").slice(1);
    var entry = byId[id];
    if (!entry) {
      var anchor = document.getElementById(id);
      if (!anchor) continue;
      // id は見出し内の空 <a class="anchor"> に付く（comrak header_ids）ので、
      // 幾何判定は親の h2/h3 で行う（見出し上端はアンカーよりわずかに上に来て判定が安定する）
      entry = byId[id] = {
        heading: anchor.closest("h1,h2,h3,h4,h5,h6") || anchor,
        items: [],
      };
      entries.push(entry);
    }
    entry.items.push(links[i].parentElement);
  }
  if (entries.length === 0) return;

  var desktopToc = document.querySelector(".toc");
  var current = null; // 現在 active な entry
  var pinned = null; // TOC クリック直後の固定。ユーザー起点のスクロール操作で解除する

  // 基準線: CSS の scroll-padding-top（getComputedStyle は px 解決済みを返す）
  // ＋サブピクセル誤差の保険。ハッシュ遷移の着地位置と点灯項目がこれで一致する
  function offsetLine() {
    var v = parseFloat(getComputedStyle(document.documentElement).scrollPaddingTop);
    return (isNaN(v) ? 0 : v) + 2;
  }

  function pick() {
    if (pinned) return pinned;
    var root = document.documentElement;
    // スクロール可能なページの最下部では最後のセクションを active にする
    //（末尾セクションが短く見出しが基準線まで届かないページへの対策）
    var scrollable = root.scrollHeight > window.innerHeight + 2;
    if (scrollable && window.innerHeight + window.scrollY >= root.scrollHeight - 2) {
      return entries[entries.length - 1];
    }
    var line = offsetLine();
    var found = null; // 先頭見出しより上（h1 と導入部）は null ＝ ハイライトなし
    for (var j = 0; j < entries.length; j++) {
      if (entries[j].heading.getBoundingClientRect().top > line) break; // 文書順なので打ち切れる
      found = entries[j];
    }
    return found;
  }

  // sticky な .toc は overflow-y: auto の独立スクロール領域なので、active 項目を可視域に保つ。
  // scrollIntoView は文書側のスクロールに連鎖しうるため scrollTop を直接調整する
  //（li.offsetTop は offsetParent = .toc 基準。閉じた details の .toc-mobile には触らない）
  function keepVisible(li) {
    if (!desktopToc || li.offsetParent !== desktopToc) return;
    if (desktopToc.scrollHeight <= desktopToc.clientHeight) return;
    var margin = 8;
    var top = li.offsetTop;
    var bottom = top + li.offsetHeight;
    if (top < desktopToc.scrollTop + margin) {
      desktopToc.scrollTop = top - margin;
    } else if (bottom > desktopToc.scrollTop + desktopToc.clientHeight - margin) {
      desktopToc.scrollTop = bottom - desktopToc.clientHeight + margin;
    }
  }

  function setActive(entry, on) {
    for (var i = 0; i < entry.items.length; i++) {
      entry.items[i].classList.toggle("active", on);
      var a = entry.items[i].querySelector("a");
      if (a) {
        if (on) a.setAttribute("aria-current", "location");
        else a.removeAttribute("aria-current");
      }
    }
  }

  function apply(next) {
    if (next === current) return;
    if (current) setActive(current, false);
    if (next) {
      setActive(next, true);
      for (var i = 0; i < next.items.length; i++) keepVisible(next.items[i]);
    }
    current = next;
  }

  var ticking = false;
  function onScroll() {
    if (ticking) return;
    ticking = true;
    requestAnimationFrame(function () {
      ticking = false;
      apply(pick());
    });
  }

  // TOC クリック → その項目を即 active にして固定する（ページ末尾に複数の見出しが
  // 収まる場合、最下部ルールが別項目を点灯させ続けるのを防ぐ）。固定の解除は
  // ユーザー起点の操作のみ（プログラムによるハッシュジャンプの scroll では発火しない）。
  // クリック時は mousedown（解除）→ click（固定）の順に発火するので固定が残る
  for (var k = 0; k < links.length; k++) {
    links[k].addEventListener("click", function (e) {
      var target = byId[e.currentTarget.getAttribute("href").slice(1)];
      if (target) {
        pinned = target;
        apply(target);
      }
    });
  }
  ["wheel", "touchstart", "mousedown", "keydown"].forEach(function (type) {
    window.addEventListener(
      type,
      function () {
        pinned = null;
      },
      { passive: true }
    );
  });

  window.addEventListener("scroll", onScroll, { passive: true });
  window.addEventListener("resize", onScroll);
  window.addEventListener("load", onScroll); // 画像・mermaid 描画後の高さ確定を拾う
  onScroll(); // 初期表示（ハッシュ付き URL・スクロール位置復元を含む）
})();
