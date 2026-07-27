// yuzu の検索 UI（DOM・キーボード操作・IME・aria 同期のみを担当）。
// フェッチ・OPFS キャッシュ・wasm 起動は _search/search-client.js（SEARCH_BASE 配下に
// 同梱される、検索エンジンと対になる手書きのクライアント）に委譲する。
// 検索エンジン・トークナイザはネイティブの `yuzu search` と同一コード。

const script = document.currentScript || document.querySelector("script[data-search-base]");
const SEARCH_BASE = script.dataset.searchBase || "/_search/";
const BASE = script.dataset.base || "/";
const DEBOUNCE_MS = 150;
// 1 回に表示する件数（`yuzu search --limit` の既定と揃えてある）。
// 残りは末尾の「さらに N 件を表示」行から追加ロードする
const PAGE_SIZE = 10;
const EXCERPT_CHARS = 160;

const input = document.getElementById("yuzu-search-input");
const resultsBox = document.getElementById("yuzu-search-results");
if (input && resultsBox) setup();

function setup() {
  // SEARCH_BASE は baseUrl 設定でページごとに変わるビルド時テンプレート値なので、
  // 静的 import ではなく動的 import で解決する（wasm グルーの読み込みと同じ理由）
  let clientPromise = null;
  let timer = null;
  let selected = -1;
  let composing = false; // IME 変換中フラグ
  let compositionEndedAt = -1; // 直前の compositionend の時刻（確定 Enter の除外用）
  let currentQuery = ""; // 追加ロードで再クエリするための現在のクエリ
  let shown = 0; // 表示中の件数（追加ロードのオフセット）
  let loading = false; // 追加ロード中（二重発火の抑止）
  // 検索の世代。debounce + 非同期なので、入力が進んだ後に古い応答が
  // 返ってくることがある。描画の直前に世代を照合して古い結果を捨てる
  let seq = 0;

  function ensureClient() {
    clientPromise ??= import(SEARCH_BASE + "search-client.js").then(({ createSearchClient }) =>
      createSearchClient({ searchBase: SEARCH_BASE }),
    );
    return clientPromise;
  }

  // "/" or Cmd/Ctrl+K でフォーカス
  document.addEventListener("keydown", (ev) => {
    const typing = /^(INPUT|TEXTAREA)$/.test(document.activeElement?.tagName || "");
    if ((ev.key === "/" && !typing) || ((ev.metaKey || ev.ctrlKey) && ev.key === "k")) {
      ev.preventDefault();
      input.focus();
    }
  });

  // 初回フォーカスでエンジンを遅延初期化（読み込み中の表示付き）
  input.addEventListener("focus", () => {
    if (!clientPromise) {
      showMessage("検索インデックスを読み込み中…");
      ensureClient()
        .then((client) => client.ensureEngine())
        .then(() => {
          // 読み込み中メッセージだけが出ている状態なら閉じる
          if (resultsBox.querySelector(".search-loading")) close();
        })
        .catch(showError);
    }
  });

  // IME 変換中は未確定文字列で検索しない（確定時に 1 回だけ実行）
  input.addEventListener("compositionstart", () => {
    composing = true;
    clearTimeout(timer);
  });
  input.addEventListener("compositionend", (ev) => {
    composing = false;
    compositionEndedAt = ev.timeStamp;
    clearTimeout(timer);
    timer = setTimeout(() => runSearch(input.value.trim()).catch(showError), DEBOUNCE_MS);
  });

  input.addEventListener("input", () => {
    if (composing) return;
    clearTimeout(timer);
    timer = setTimeout(() => runSearch(input.value.trim()).catch(showError), DEBOUNCE_MS);
  });

  input.addEventListener("keydown", (ev) => {
    // IME 変換中のキー操作（候補の移動・確定）を奪わない
    if (ev.isComposing || ev.keyCode === 229) return;
    // 「さらに N 件を表示」行も option なので、矢印キーの循環に自然に入る
    //（キーボードだけで「まだ続きがある」ことに気づける）
    const items = optionItems();
    if (ev.key === "ArrowDown" || ev.key === "ArrowUp") {
      ev.preventDefault();
      if (!items.length) return;
      selected = ev.key === "ArrowDown"
        ? (selected + 1) % items.length
        : (selected - 1 + items.length) % items.length;
      updateSelection(items);
      items[selected].scrollIntoView({ block: "nearest" });
    } else if (ev.key === "Enter" && items.length) {
      // 未選択の Enter は先頭ヒットへ（コンボボックスの一般的挙動）。
      // ただし Safari は IME 確定の Enter を compositionend の後に
      // isComposing: false の素の keydown として発火するため、同一キーストローク
      // 由来（compositionend と時刻が近接）の Enter は遷移させない。
      // **この IME ガードは追加ロードの分岐より前**（逆にすると日本語を確定した
      // 瞬間に勝手に追加ロードが走る）
      if (ev.timeStamp - compositionEndedAt < 100) return;
      const target = items[Math.max(selected, 0)];
      // more 行には href が無い。分岐を忘れると /undefined へ遷移する
      if (target.classList.contains("search-more")) {
        ev.preventDefault();
        loadMore(true);
        return;
      }
      location.href = target.href;
    } else if (ev.key === "Escape") {
      close();
      input.blur();
    }
  });

  document.addEventListener("click", (ev) => {
    if (!ev.target.closest("#yuzu-search")) close();
  });

  // 選択対象（ヒット行と「さらに N 件を表示」行）
  function optionItems() {
    return resultsBox.querySelectorAll('[role="option"]');
  }

  // 選択状態を class と aria（aria-selected / aria-activedescendant）へ同期する
  function updateSelection(items) {
    items.forEach((el, i) => {
      el.classList.toggle("selected", i === selected);
      el.setAttribute("aria-selected", i === selected ? "true" : "false");
    });
    if (selected >= 0 && items[selected]) {
      input.setAttribute("aria-activedescendant", items[selected].id);
    } else {
      input.removeAttribute("aria-activedescendant");
    }
  }

  async function runSearch(query) {
    const my = ++seq; // 新しい検索。実行中の古い検索・追加ロードは無効になる
    if (!query) {
      close();
      return;
    }
    currentQuery = query;
    const client = await ensureClient();
    const { total, hits } = await client.search(query, PAGE_SIZE);
    const fragments = await Promise.all(hits.map((h) => client.fetchFragment(h.docId)));
    if (my !== seq) return; // 入力が進んでいる = この結果はもう古い
    shown = fragments.length;
    render(client, query, fragments, total, 0);
  }

  // 「さらに N 件を表示」。limit を増やして再クエリし、増えた分だけ追記する。
  // エンジンの並びは (スコア降順, doc_id 昇順) の全順序で切り詰めるだけなので、
  // limit を増やした結果は前回の結果を必ず接頭辞として含む = 追記して整合する
  // fromKeyboard: Enter 由来なら追加分の先頭へ選択を進める（クリック由来では動かさない）
  async function loadMore(fromKeyboard) {
    const more = resultsBox.querySelector(".search-more");
    if (!more || loading) return;
    loading = true;
    const my = seq;
    const offset = shown;
    more.setAttribute("aria-disabled", "true");
    more.textContent = "読み込み中…";
    try {
      const client = await ensureClient();
      const { total, hits } = await client.search(currentQuery, offset + PAGE_SIZE);
      // fragment はクライアント側でメモ化されているので、実際に取りに行くのは増えた分だけ
      const fragments = await Promise.all(hits.map((h) => client.fetchFragment(h.docId)));
      if (my !== seq) return; // クエリが変わった = この追加分は捨てる
      shown = fragments.length;
      render(client, currentQuery, fragments, total, offset);
      const items = optionItems();
      if (fromKeyboard && items[offset]) {
        selected = offset;
        updateSelection(items);
        items[selected].scrollIntoView({ block: "nearest" });
      }
    } catch (err) {
      if (my !== seq) return;
      // 結果全体は消さない（showError は箱ごと差し替えてしまう）
      console.error("[yuzu-search]", err);
      more.removeAttribute("aria-disabled");
      more.textContent = "読み込めませんでした（もう一度）";
    } finally {
      loading = false;
    }
  }

  // offset === 0 は新しい検索（箱をクリア）、offset > 0 は追加ロード（追記）。
  // 追記のときは DOM を消さないので、スクロール位置と選択状態がそのまま残る
  function render(client, query, fragments, total, offset) {
    if (offset === 0) {
      selected = -1;
      input.removeAttribute("aria-activedescendant");
      resultsBox.innerHTML = "";
      if (!fragments.length) {
        // クエリ文字列は textContent 経由で入れる（XSS 安全）
        const empty = document.createElement("div");
        empty.className = "search-empty";
        empty.textContent = `「${query}」に一致するページはありません`;
        resultsBox.append(empty);
        appendHint(query);
        open();
        return;
      }
      const count = document.createElement("div");
      count.className = "search-count";
      resultsBox.append(count);
    } else {
      // 末尾の飾り（more 行・ヒント）は付け直す
      resultsBox.querySelector(".search-more")?.remove();
      resultsBox.querySelector(".search-hint")?.remove();
    }
    const count = resultsBox.querySelector(".search-count");
    if (count) {
      count.textContent =
        total > fragments.length
          ? `${total} 件（上位 ${fragments.length} 件を表示）`
          : `${total} 件`;
    }
    for (const [i, fragment] of fragments.slice(offset).entries()) {
      const a = document.createElement("a");
      a.className = "search-hit";
      a.id = `yuzu-search-hit-${offset + i}`;
      a.setAttribute("role", "option");
      a.setAttribute("aria-selected", "false");
      // セクション doc は見出しアンカーへ直接ジャンプする
      a.href = BASE + fragment.url + (fragment.anchor ? "#" + fragment.anchor : "");
      const title = document.createElement("div");
      title.className = "search-hit-title";
      title.append(...markSegments(client, fragment.title, query));
      if (fragment.heading) {
        const crumb = document.createElement("span");
        crumb.className = "search-hit-crumb";
        crumb.append(" › ", ...markSegments(client, fragment.heading, query));
        title.append(crumb);
      }
      const excerpt = document.createElement("div");
      excerpt.className = "search-hit-excerpt";
      excerpt.append(...markSegments(client, fragment.text, query, EXCERPT_CHARS));
      a.append(title, excerpt);
      resultsBox.append(a);
    }
    if (total > fragments.length) appendMore(total, fragments.length);
    appendHint(query);
    open();
  }

  // 追加ロード行。button ではなく option にするのは、listbox の中に
  // interactive 要素を入れず（Tab フォーカスが input から逃げない）、
  // aria-activedescendant の対象にもできるため
  function appendMore(total, shownCount) {
    const rest = total - shownCount;
    const more = document.createElement("div");
    more.className = "search-more";
    more.id = "yuzu-search-more";
    more.setAttribute("role", "option");
    more.setAttribute("aria-selected", "false");
    more.textContent = `さらに ${Math.min(PAGE_SIZE, rest)} 件を表示（残り ${rest} 件）`;
    // 引数付きで呼ぶ（addEventListener に直接渡すと MouseEvent が第 1 引数に入る）
    more.addEventListener("click", () => loadMore(false));
    resultsBox.append(more);
  }

  // フレーズ検索の発見用ヒント（引用符を既に使っているクエリでは出さない）
  function appendHint(query) {
    if (/["＂“”]/.test(query)) return;
    const hint = document.createElement("div");
    hint.className = "search-hint";
    hint.textContent = '"..." で囲むと完全一致（フレーズ）検索';
    resultsBox.append(hint);
  }

  // wasm の excerpt（エンジンと同一の分かち書き・正規化）で <mark> 断片列を作る。
  // XSS 安全: 文字列は必ず createTextNode / textContent 経由で DOM 化する。
  // maxChars 既定 10000 = タイトル用の実質切り詰めなし（一致がなければ原文のまま）
  function markSegments(client, text, query, maxChars = 10000) {
    const segments = client.excerpt(text, query, maxChars);
    return segments.map((seg) => {
      if (!seg.mark) return document.createTextNode(seg.text);
      const mark = document.createElement("mark");
      mark.textContent = seg.text;
      return mark;
    });
  }

  function open() {
    resultsBox.hidden = false;
    input.setAttribute("aria-expanded", "true");
  }

  function close() {
    resultsBox.hidden = true;
    input.setAttribute("aria-expanded", "false");
    input.removeAttribute("aria-activedescendant");
    selected = -1;
  }

  // 一時メッセージ（読み込み中等）。検索結果が来たら render が上書きする
  function showMessage(text) {
    resultsBox.innerHTML = "";
    const div = document.createElement("div");
    div.className = "search-empty search-loading";
    div.textContent = text;
    resultsBox.append(div);
    open();
  }

  function showError(err) {
    console.error("[yuzu-search]", err);
    resultsBox.innerHTML = `<div class="search-empty">検索を初期化できませんでした（コンソール参照）</div>`;
    open();
  }
}
