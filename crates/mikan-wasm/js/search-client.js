// フェッチ ＋ OPFS キャッシュ ＋ wasm 起動のオーケストレーション。
// DOM・テーマの知識は一切持たない（yuzu-theme の search-ui.js から利用される）。
//
// キャッシュ有効性判定: manifest.json は毎回ネットワークから取得し（軽量・常に
// 新鮮なものを使う）、contentHash が OPFS 保存済みの前回 manifest（version marker
// 用途のみ・生きたソースにはしない）と一致すれば terms.fst/model.zst/シャードを
// OPFS から読む。不一致・未保存なら OPFS 名前空間を丸ごと作り直してから取得し直す
// （term_id は語彙順の連番のため、部分無効化ではなく全体破棄が安全でシンプル）。
//
// OPFS 関連の処理は丸ごと try/catch で囲み、何が起きても検索自体（フェッチのみ経路）
// は継続する。fragment は対象外（クエリごとに異なる doc_id を指しセッション跨ぎの
// ヒット率が低いため。メモリ内キャッシュで足りている）。

import { openNamespace } from "./opfs-cache.js";

/**
 * @param {{ searchBase: string }} opts
 */
export function createSearchClient({ searchBase }) {
  let engine = null;
  let enginePromise = null;
  const shardCache = new Set();
  const fragmentCache = new Map();

  async function fetchBytes(rel) {
    const res = await fetch(searchBase + rel);
    if (!res.ok) throw new Error(`fetch ${rel}: ${res.status}`);
    return new Uint8Array(await res.arrayBuffer());
  }

  // OPFS にあれば読み、無ければネットワークから取得して書き込む（cache が
  // null なら常にネットワークから取得するだけの従来どおりの経路になる）
  async function loadCached(cache, rel) {
    if (cache) {
      const hit = await cache.get(rel);
      if (hit) return hit;
    }
    const bytes = await fetchBytes(rel);
    if (cache) await cache.put(rel, bytes);
    return bytes;
  }

  async function resolveCache(manifestBytes) {
    try {
      const cache = await openNamespace(searchBase);
      if (!cache) return null;
      const prevBytes = await cache.get("manifest.json");
      const prevHash = prevBytes ? tryReadContentHash(prevBytes) : null;
      const contentHash = tryReadContentHash(manifestBytes);
      if (prevHash !== contentHash) {
        await cache.clear();
      }
      // 新鮮な manifest を version marker として保存する（次回訪問時の比較用。
      // エンジン構築には常にこの関数の呼び出し元が取得した新鮮なバイト列を使う）
      await cache.put("manifest.json", manifestBytes);
      return cache;
    } catch {
      return null; // OPFS で何が起きても検索自体は継続する
    }
  }

  function tryReadContentHash(bytes) {
    try {
      return JSON.parse(new TextDecoder().decode(bytes)).contentHash ?? null;
    } catch {
      return null;
    }
  }

  async function ensureEngine() {
    if (engine) return engine;
    enginePromise ??= (async () => {
      const [mod, manifestBytes] = await Promise.all([
        import(searchBase + "search.js"),
        fetchBytes("manifest.json"),
      ]);
      const cache = await resolveCache(manifestBytes);
      const [terms, model] = await Promise.all([
        loadCached(cache, "terms.fst"),
        loadCached(cache, "model.zst"),
      ]);
      await mod.default({ module_or_path: searchBase + "search_bg.wasm" });
      engine = { instance: new mod.YuzuSearch(manifestBytes, terms, model), cache };
      return engine;
    })();
    return enginePromise;
  }

  async function search(query, limit) {
    const { instance, cache } = await ensureEngine();
    const needed = Array.from(instance.neededShards(query)).filter((id) => !shardCache.has(id));
    await Promise.all(
      needed.map(async (id) => {
        const rel = `index/${String(id).padStart(4, "0")}.bin`;
        const bytes = await loadCached(cache, rel);
        instance.loadShard(id, bytes);
        shardCache.add(id);
      }),
    );
    return JSON.parse(instance.search(query, limit));
  }

  async function fetchFragment(docId) {
    if (!fragmentCache.has(docId)) {
      const res = await fetch(searchBase + `fragment/${docId}.json`);
      fragmentCache.set(docId, await res.json());
    }
    return fragmentCache.get(docId);
  }

  // エンジンが既にロード済みであることが前提（ensureEngine/search を先に呼ぶこと）。
  // wasm 側の同期呼び出しをそのまま公開する薄いラッパ
  function tokenize(query) {
    if (!engine) throw new Error("ensureEngine() を先に呼んでください");
    return JSON.parse(engine.instance.tokenize(query));
  }

  function excerpt(text, query, maxChars) {
    if (!engine) throw new Error("ensureEngine() を先に呼んでください");
    return JSON.parse(engine.instance.excerpt(text, query, maxChars));
  }

  return { ensureEngine, search, fetchFragment, tokenize, excerpt };
}
