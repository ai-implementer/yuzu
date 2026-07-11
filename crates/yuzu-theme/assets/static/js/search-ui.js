// yuzu の検索 UI（Pagefind 型の 2 段フェッチ）。
// wasm（純計算）は _search/search.js 経由で遅延ロードし、
// manifest / terms.fst / model.zst → 必要シャード → 上位ヒットの fragment の順に fetch する。
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
  let engine = null;
  let enginePromise = null;
  const shardCache = new Set();
  const fragmentCache = new Map();
  let timer = null;
  let selected = -1;

  // "/" or Cmd/Ctrl+K でフォーカス
  document.addEventListener("keydown", (ev) => {
    const typing = /^(INPUT|TEXTAREA)$/.test(document.activeElement?.tagName || "");
    if ((ev.key === "/" && !typing) || ((ev.metaKey || ev.ctrlKey) && ev.key === "k")) {
      ev.preventDefault();
      input.focus();
    }
  });

  // 初回フォーカスでエンジンを遅延初期化
  input.addEventListener("focus", () => ensureEngine().catch(showError));

  input.addEventListener("input", () => {
    clearTimeout(timer);
    timer = setTimeout(() => runSearch(input.value.trim()).catch(showError), DEBOUNCE_MS);
  });

  input.addEventListener("keydown", (ev) => {
    const items = resultsBox.querySelectorAll("a.search-hit");
    if (ev.key === "ArrowDown" || ev.key === "ArrowUp") {
      ev.preventDefault();
      if (!items.length) return;
      selected = ev.key === "ArrowDown"
        ? (selected + 1) % items.length
        : (selected - 1 + items.length) % items.length;
      items.forEach((el, i) => el.classList.toggle("selected", i === selected));
      items[selected].scrollIntoView({ block: "nearest" });
    } else if (ev.key === "Enter" && selected >= 0 && items[selected]) {
      location.href = items[selected].href;
    } else if (ev.key === "Escape") {
      close();
      input.blur();
    }
  });

  document.addEventListener("click", (ev) => {
    if (!ev.target.closest("#yuzu-search")) close();
  });

  async function ensureEngine() {
    if (engine) return engine;
    enginePromise ??= (async () => {
      const [mod, manifest, terms, model] = await Promise.all([
        import(SEARCH_BASE + "search.js"),
        fetchBytes("manifest.json"),
        fetchBytes("terms.fst"),
        fetchBytes("model.zst"),
      ]);
      await mod.default({ module_or_path: SEARCH_BASE + "search_bg.wasm" });
      engine = { mod, instance: new mod.YuzuSearch(manifest, terms, model) };
      return engine;
    })();
    return enginePromise;
  }

  async function runSearch(query) {
    if (!query) {
      close();
      return;
    }
    const { instance } = await ensureEngine();

    // 未取得シャードだけ fetch して登録
    const needed = Array.from(instance.neededShards(query)).filter((id) => !shardCache.has(id));
    await Promise.all(
      needed.map(async (id) => {
        const bytes = await fetchBytes(`index/${String(id).padStart(4, "0")}.bin`);
        instance.loadShard(id, bytes);
        shardCache.add(id);
      }),
    );

    const hits = JSON.parse(instance.search(query, LIMIT));
    const fragments = await Promise.all(hits.map((h) => fetchFragment(h.docId)));
    render(query, instance, fragments);
  }

  async function fetchFragment(docId) {
    if (!fragmentCache.has(docId)) {
      const res = await fetch(SEARCH_BASE + `fragment/${docId}.json`);
      fragmentCache.set(docId, await res.json());
    }
    return fragmentCache.get(docId);
  }

  function render(query, instance, fragments) {
    selected = -1;
    resultsBox.innerHTML = "";
    if (!fragments.length) {
      resultsBox.innerHTML = `<div class="search-empty">一致するページはありません</div>`;
      open();
      return;
    }
    for (const fragment of fragments) {
      const a = document.createElement("a");
      a.className = "search-hit";
      // セクション doc は見出しアンカーへ直接ジャンプする
      a.href = BASE + fragment.url + (fragment.anchor ? "#" + fragment.anchor : "");
      const title = document.createElement("div");
      title.className = "search-hit-title";
      title.append(...markSegments(instance, fragment.title, query));
      if (fragment.heading) {
        const crumb = document.createElement("span");
        crumb.className = "search-hit-crumb";
        crumb.append(" › ", ...markSegments(instance, fragment.heading, query));
        title.append(crumb);
      }
      const excerpt = document.createElement("div");
      excerpt.className = "search-hit-excerpt";
      excerpt.append(...markSegments(instance, fragment.text, query, EXCERPT_CHARS));
      a.append(title, excerpt);
      resultsBox.append(a);
    }
    open();
  }

  // wasm の excerpt（エンジンと同一の分かち書き・正規化）で <mark> 断片列を作る。
  // XSS 安全: 文字列は必ず createTextNode / textContent 経由で DOM 化する。
  // maxChars 既定 10000 = タイトル用の実質切り詰めなし（一致がなければ原文のまま）
  function markSegments(instance, text, query, maxChars = 10000) {
    const segments = JSON.parse(instance.excerpt(text, query, maxChars));
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
    selected = -1;
  }

  function showError(err) {
    console.error("[yuzu-search]", err);
    resultsBox.innerHTML = `<div class="search-empty">検索を初期化できませんでした（コンソール参照）</div>`;
    open();
  }

  async function fetchBytes(rel) {
    const res = await fetch(SEARCH_BASE + rel);
    if (!res.ok) throw new Error(`fetch ${rel}: ${res.status}`);
    return new Uint8Array(await res.arrayBuffer());
  }
}
