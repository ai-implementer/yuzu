// 検索結果ページ（search.jinja の #yuzu-search-page）の UI。
//
// **状態の持ち主は URL（?q= / ?section=）のみ** — sessionStorage は使わない。
// URL を共有したとき受け手で同じ結果が再現することがこのページの目的なので、
// ローカル状態を混ぜると「共有したのに前回の絞り込みが復活する」事故になる。
// フェッチ・OPFS キャッシュ・wasm 起動はドロップダウンと同じ
// _search/search-client.js に委譲する（エンジンはネイティブの `yuzu search` と同一）
import { hintRow, hitLink } from "./search-hits.js";

// type="module" では document.currentScript が null になるため、
// script タグではなくコンテナ div の data-* から設定を受ける
// （script[data-search-base] は search-ui.js のタグと衝突する）
const root = document.getElementById("yuzu-search-page");
if (root) setup(root);

function setup(root) {
  const SEARCH_BASE = root.dataset.searchBase || "/_search/";
  const BASE = root.dataset.base || "/";
  // `search.page_size`（テンプレート経由）。1 回に表示する件数
  const PAGE_SIZE = Math.max(1, parseInt(root.dataset.pageSize, 10) || 10);
  const DEBOUNCE_MS = 150;

  const input = document.getElementById("yuzu-search-page-input");
  const form = root.querySelector(".search-page-form");
  const filtersBox = root.querySelector(".search-page-filters");
  const statusBox = root.querySelector(".search-page-status");
  const resultsBox = root.querySelector(".search-page-results");
  const moreBtn = root.querySelector(".search-page-more");
  if (!input || !form || !filtersBox || !statusBox || !resultsBox || !moreBtn) return;

  let clientPromise = null;
  let timer = null;
  let composing = false;
  let seq = 0; // 世代照合（古い応答を捨てる）。ドロップダウンと同じ流儀
  let groupNames = []; // インデックス由来の区分名（ナビ順）。空 = 絞り込み非対応
  let query = "";
  let sections = []; // 絞り込み中の区分（表示名）。URL の ?section= と常に一致
  let limit = PAGE_SIZE; // 「さらに表示」で += PAGE_SIZE
  let shown = 0; // 表示中の件数（追記のオフセット）

  function ensureClient() {
    clientPromise ??= import(SEARCH_BASE + "search-client.js").then(({ createSearchClient }) =>
      createSearchClient({ searchBase: SEARCH_BASE }),
    );
    return clientPromise;
  }

  function readUrl() {
    const params = new URLSearchParams(location.search);
    query = (params.get("q") || "").trim();
    sections = params.getAll("section").filter(Boolean);
  }

  // q / section を URL へ反映する。replaceState = 履歴を増やさない
  // （入力のたびに履歴が積まれると「戻る」が検索の途中経過を延々と辿ることになる。
  // 「戻る」は検索前のページへ、が意図した動き）
  function writeUrl() {
    const params = new URLSearchParams();
    if (query) params.set("q", query);
    for (const s of sections) params.append("section", s);
    const qs = params.toString();
    history.replaceState(null, "", qs ? "?" + qs : location.pathname);
  }

  form.addEventListener("submit", (ev) => {
    // JS ありなら遷移せずその場で実行（JS なしでは素の GET で ?q= 遷移になる）
    ev.preventDefault();
    clearTimeout(timer);
    setQuery(input.value.trim());
  });

  // IME 変換中は未確定文字列で検索しない（ドロップダウンと同じ）
  input.addEventListener("compositionstart", () => {
    composing = true;
    clearTimeout(timer);
  });
  input.addEventListener("compositionend", () => {
    composing = false;
    clearTimeout(timer);
    timer = setTimeout(() => setQuery(input.value.trim()), DEBOUNCE_MS);
  });
  input.addEventListener("input", () => {
    if (composing) return;
    clearTimeout(timer);
    timer = setTimeout(() => setQuery(input.value.trim()), DEBOUNCE_MS);
  });

  moreBtn.addEventListener("click", () => {
    limit += PAGE_SIZE;
    run({ append: true }).catch(showError);
  });

  function setQuery(next) {
    if (next === query) return;
    query = next;
    limit = PAGE_SIZE;
    writeUrl();
    run().catch(showError);
  }

  function toggleSection(name) {
    sections = sections.includes(name)
      ? sections.filter((s) => s !== name)
      : [...sections, name];
    limit = PAGE_SIZE;
    writeUrl();
    run().catch(showError);
  }

  async function run({ append = false } = {}) {
    const my = ++seq;
    if (!query) {
      filtersBox.hidden = true;
      filtersBox.innerHTML = "";
      statusBox.textContent = "";
      resultsBox.innerHTML = "";
      moreBtn.hidden = true;
      return;
    }
    if (!append) statusBox.textContent = "検索中…";
    const client = await ensureClient();
    if (!groupNames.length) {
      // 区分名は最初の検索より前に取る。URL 由来の ?section= にインデックスに
      // 無い名前（改名・削除・打ち間違い）が混ざったまま検索すると、その区分の
      // ヒットが黙って 0 になり「共有したのに結果が違う」に見えるため、
      // 先に捨てて URL へも反映する
      await client.ensureEngine();
      groupNames = client.groups();
      const alive = sections.filter((s) => groupNames.includes(s));
      if (alive.length !== sections.length) {
        sections = alive;
        writeUrl();
      }
    }
    const result = await client.search(query, limit, sections);
    const { total, hits } = result;
    // fragment はクライアント側でメモ化されているので、追記時に取りに行くのは増えた分だけ
    const fragments = await Promise.all(hits.map((h) => client.fetchFragment(h.docId)));
    if (my !== seq) return; // クエリ・絞り込みが進んでいる = この結果はもう古い
    renderFilters(result.groupCounts);
    render(client, fragments, total, append ? shown : 0, result.totalUnfiltered);
    shown = fragments.length;
  }

  // 区分チップ。件数は**絞り込み前**の値（選んでも数字が動かない = 押す前に
  // 何件あるか分かる）。ヒットのある区分が 2 つ未満なら行ごと出さない
  // = 階層の無いサイト・古いインデックス・旧 wasm がすべてここに落ちる。
  // 結果ページは listbox 制約が無いので、チップは普通の button でよい
  function renderFilters(counts) {
    const at = (i) => (Array.isArray(counts) ? (counts[i] ?? 0) : 0);
    const live = groupNames.filter((_, i) => at(i) > 0).length;
    if (live < 2) {
      filtersBox.hidden = true;
      filtersBox.innerHTML = "";
      return;
    }
    filtersBox.innerHTML = "";
    const chip = (label, active, onClick, count) => {
      const b = document.createElement("button");
      b.type = "button";
      b.className = "search-filter-chip";
      b.setAttribute("aria-pressed", active ? "true" : "false");
      b.append(document.createTextNode(label));
      if (count !== undefined) {
        const n = document.createElement("span");
        n.className = "search-filter-count";
        n.textContent = String(count);
        b.append(n);
      }
      b.addEventListener("click", onClick);
      filtersBox.append(b);
    };
    chip("すべて", sections.length === 0, () => {
      if (!sections.length) return;
      sections = [];
      limit = PAGE_SIZE;
      writeUrl();
      run().catch(showError);
    });
    groupNames.forEach((name, i) => {
      if (!at(i)) return; // ヒット 0 の区分は出さない
      chip(name, sections.includes(name), () => toggleSection(name), at(i));
    });
    filtersBox.hidden = false;
  }

  // offset === 0 は新しい検索（箱をクリア）、offset > 0 は「さらに表示」（追記）。
  // limit を増やした結果は前回の結果を必ず接頭辞として含む（エンジンの並びは
  // (スコア降順, doc_id 昇順) の全順序で切り詰めるだけ）ので追記して整合する。
  // ⚠️ 再クエリには現在の絞り込みを必ず渡すこと（渡し忘れると接頭辞性が崩れる）
  function render(client, fragments, total, offset, totalUnfiltered) {
    if (offset === 0) {
      resultsBox.innerHTML = "";
    } else {
      resultsBox.querySelector(".search-hint")?.remove();
    }
    if (!fragments.length) {
      statusBox.textContent = `「${query}」に一致するページはありません`;
      appendHint();
      moreBtn.hidden = true;
      return;
    }
    // 絞り込み中は全体の件数も添える（何を絞ったのかが分かる）
    const scope =
      sections.length && totalUnfiltered > total ? `・全体 ${totalUnfiltered} 件` : "";
    statusBox.textContent =
      total > fragments.length
        ? `${total} 件（上位 ${fragments.length} 件を表示${scope}）`
        : `${total} 件${scope ? `（${scope.slice(1)}）` : ""}`;
    for (const fragment of fragments.slice(offset)) {
      resultsBox.append(hitLink(client, fragment, query, BASE));
    }
    appendHint();
    const rest = total - fragments.length;
    moreBtn.hidden = rest <= 0;
    if (rest > 0) {
      moreBtn.textContent = `さらに ${Math.min(PAGE_SIZE, rest)} 件を表示（残り ${rest} 件）`;
    }
  }

  function appendHint() {
    const hint = hintRow(query);
    if (hint) resultsBox.append(hint);
  }

  function showError(err) {
    console.error("[yuzu-search]", err);
    statusBox.textContent = "検索を初期化できませんでした（コンソール参照）";
    moreBtn.hidden = true;
  }

  // 初期化: URL が正。q があれば即実行、無ければ入力へフォーカス
  // （検索しに来たページなのでフォーカスを奪ってよい — ドロップダウンとは逆）
  readUrl();
  input.value = query;
  if (query) {
    run().catch(showError);
  } else {
    input.focus();
  }
}
