// yuzu の検索 UI（DOM・キーボード操作・IME・aria 同期のみを担当）。
// フェッチ・OPFS キャッシュ・wasm 起動は _search/search-client.js（SEARCH_BASE 配下に
// 同梱される、検索エンジンと対になる手書きのクライアント）に委譲する。
// 検索エンジン・トークナイザはネイティブの `yuzu search` と同一コード。

const script = document.currentScript || document.querySelector("script[data-search-base]");
const SEARCH_BASE = script.dataset.searchBase || "/_search/";
const BASE = script.dataset.base || "/";
const DEBOUNCE_MS = 150;
const LIMIT = 10;
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
    const items = resultsBox.querySelectorAll("a.search-hit");
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
      // 由来（compositionend と時刻が近接）の Enter は遷移させない
      if (ev.timeStamp - compositionEndedAt < 100) return;
      location.href = items[Math.max(selected, 0)].href;
    } else if (ev.key === "Escape") {
      close();
      input.blur();
    }
  });

  document.addEventListener("click", (ev) => {
    if (!ev.target.closest("#yuzu-search")) close();
  });

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
    if (!query) {
      close();
      return;
    }
    const client = await ensureClient();
    const { total, hits } = await client.search(query, LIMIT);
    const fragments = await Promise.all(hits.map((h) => client.fetchFragment(h.docId)));
    render(client, query, fragments, total);
  }

  function render(client, query, fragments, total) {
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
    count.textContent =
      total > fragments.length ? `${total} 件（上位 ${fragments.length} 件を表示）` : `${total} 件`;
    resultsBox.append(count);
    for (const [i, fragment] of fragments.entries()) {
      const a = document.createElement("a");
      a.className = "search-hit";
      a.id = `yuzu-search-hit-${i}`;
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
    appendHint(query);
    open();
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
