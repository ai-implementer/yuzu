// yuzu の検索サジェスト（DOM・キーボード操作・IME・aria 同期のみを担当）。
// フェッチ・OPFS キャッシュ・wasm 起動は _search/search-client.js（SEARCH_BASE 配下に
// 同梱される、検索エンジンと対になる手書きのクライアント）に委譲する。
// 検索エンジン・トークナイザはネイティブの `yuzu search` と同一コード。
//
// Phase 54 でサジェスト専用へ簡素化した。絞り込み・追加ロード・遷移後復元
// （Phase 53 の sessionStorage 群）は検索結果ページ（`search.page` /
// search-page.js。状態の持ち主は URL のみ）へ一本化し、ここは上位数件＋
// 「すべての結果を見る」行だけを出す
import { hintRow, hitLink } from "./search-hits.js";

// type="module" では document.currentScript が null になるため属性で引く
const script = document.querySelector("script[data-search-base]");
const SEARCH_BASE = script?.dataset.searchBase || "/_search/";
const BASE = script?.dataset.base || "/";
// 検索結果ページの URL。`search.page` 未設定なら空 = 「すべての結果を見る」行を出さない
const SEARCH_PAGE = script?.dataset.searchPage || "";
const DEBOUNCE_MS = 150;
// サジェストの表示件数（固定）。全結果は検索結果ページで見る
const SUGGEST_LIMIT = 5;

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
    // 「すべての結果を見る」行も option なので、矢印キーの循環に自然に入る
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
      // 由来（compositionend と時刻が近接）の Enter は遷移させない
      if (ev.timeStamp - compositionEndedAt < 100) return;
      // 行はすべて href を持つ実アンカー（ヒット・「すべての結果を見る」）
      location.href = items[Math.max(selected, 0)].href;
    } else if (ev.key === "Escape") {
      close();
      input.blur();
    }
  });

  // 検索 UI のルート（外側クリック判定の基準）
  const searchRoot = input.closest("#yuzu-search") ?? resultsBox.parentElement;

  document.addEventListener("click", (ev) => {
    // ⚠️ 判定に `ev.target.closest()` を使ってはいけない。**押した要素が
    // ハンドラ内の再描画で DOM から外れている**ことがあり、外れた要素の
    // closest は必ず null になるため「外側のクリック」と誤判定して検索を
    // 閉じてしまう。composedPath はディスパッチ時の経路を保持するので
    // 切り離しの影響を受けない
    const path = ev.composedPath?.() ?? [];
    const inside = path.length
      ? path.includes(searchRoot)
      : Boolean(ev.target.closest?.("#yuzu-search"));
    if (!inside) close();
  });

  // 選択対象（ヒット行と「すべての結果を見る」行）
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
    const my = ++seq; // 新しい検索。実行中の古い検索は無効になる
    if (!query) {
      close();
      return;
    }
    const client = await ensureClient();
    const { total, hits } = await client.search(query, SUGGEST_LIMIT, []);
    const fragments = await Promise.all(hits.map((h) => client.fetchFragment(h.docId)));
    if (my !== seq) return; // 入力が進んでいる = この結果はもう古い
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
      total > fragments.length
        ? `${total} 件（上位 ${fragments.length} 件を表示）`
        : `${total} 件`;
    resultsBox.append(count);
    for (const [i, fragment] of fragments.entries()) {
      const a = hitLink(client, fragment, query, BASE);
      a.id = `yuzu-search-hit-${i}`;
      a.setAttribute("role", "option");
      a.setAttribute("aria-selected", "false");
      resultsBox.append(a);
    }
    appendAllResults(query, total);
    appendHint(query);
    open();
  }

  // 「すべての結果を見る」行。href を持つ実アンカーなので Enter の特別分岐が要らない。
  // button ではなく option にするのは listbox の中に interactive 要素を入れず
  // （Tab フォーカスが input から逃げない）、aria-activedescendant の対象にもできるため
  function appendAllResults(query, total) {
    if (!SEARCH_PAGE) return;
    const a = document.createElement("a");
    a.className = "search-all";
    a.id = "yuzu-search-all";
    a.setAttribute("role", "option");
    a.setAttribute("aria-selected", "false");
    a.href = SEARCH_PAGE + "?q=" + encodeURIComponent(query);
    a.textContent = `すべての結果を見る（全 ${total} 件）`;
    resultsBox.append(a);
  }

  function appendHint(query) {
    const hint = hintRow(query);
    if (hint) resultsBox.append(hint);
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
