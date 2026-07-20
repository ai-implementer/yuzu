// 汎用 OPFS（Origin Private File System）ブロブキャッシュ。
// 検索インデックスのフォーマットは一切知らない（フラットな path → bytes の
// キーバリューストアとしてのみ振る舞う）。
//
// navigator.storage.getDirectory が無い、またはいずれかの操作が一度でも
// 失敗したら、このページセッションでは恒久的に無効化して null を返す
// （リトライや部分的な機能維持はしない）。呼び出し側は null を「キャッシュなし」
// として扱い、既存のフェッチのみ経路へ縮退すること。

let disabled = false;

// OPFS のファイル名として安全な文字だけに正規化する（"/" を含む path も
// 1 ファイルのキーとしてそのままサニタイズする＝実ディレクトリは作らない）
function sanitize(raw) {
  const cleaned = String(raw).replace(/[^A-Za-z0-9_.-]/g, "_");
  return cleaned || "_";
}

/**
 * 名前空間（サイトごとに分離されたキャッシュ領域）を開く。
 * @param {string} rawKey 名前空間キー（例: SEARCH_BASE）
 * @returns {Promise<{
 *   get: (path: string) => Promise<Uint8Array|null>,
 *   put: (path: string, bytes: Uint8Array) => Promise<void>,
 *   clear: () => Promise<void>,
 * } | null>} OPFS が使えない場合は null
 */
export async function openNamespace(rawKey) {
  if (disabled || !navigator.storage?.getDirectory) return null;
  try {
    const root = await navigator.storage.getDirectory();
    const app = await root.getDirectoryHandle("yuzu-search", { create: true });
    const ns = await app.getDirectoryHandle(sanitize(rawKey), { create: true });
    return {
      get: (path) => getBytes(ns, path),
      put: (path, bytes) => putBytes(ns, path, bytes),
      clear: () => clearAll(ns),
    };
  } catch {
    disabled = true;
    return null;
  }
}

async function getBytes(ns, path) {
  if (disabled) return null;
  try {
    const handle = await ns.getFileHandle(sanitize(path));
    const blob = await handle.getFile();
    return new Uint8Array(await blob.arrayBuffer());
  } catch {
    // 未キャッシュ（NotFoundError 含む）。破損扱いにはせず単に「無い」ものとして返す
    return null;
  }
}

async function putBytes(ns, path, bytes) {
  if (disabled) return;
  try {
    const handle = await ns.getFileHandle(sanitize(path), { create: true });
    const writable = await handle.createWritable();
    await writable.write(bytes);
    await writable.close();
  } catch {
    disabled = true;
  }
}

async function clearAll(ns) {
  if (disabled) return;
  try {
    for await (const name of ns.keys()) {
      await ns.removeEntry(name, { recursive: true });
    }
  } catch {
    disabled = true;
  }
}
