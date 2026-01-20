import { invoke } from '@tauri-apps/api/core';

// L1: In-memory cache for instant retrieval
const memoryCache = new Map<string, string | null>();
const pending = new Map<string, Promise<string | null>>();

// L2: IndexedDB for persistent storage across sessions
const DB_NAME = 'TextureThumbnailCache';
const DB_VERSION = 1;
const STORE_NAME = 'thumbnails';

let dbPromise: Promise<IDBDatabase> | null = null;

function openDB(): Promise<IDBDatabase> {
  if (dbPromise) return dbPromise;

  dbPromise = new Promise((resolve, reject) => {
    const request = indexedDB.open(DB_NAME, DB_VERSION);

    request.onerror = () => reject(request.error);
    request.onsuccess = () => resolve(request.result);

    request.onupgradeneeded = (event) => {
      const db = (event.target as IDBOpenDBRequest).result;
      if (!db.objectStoreNames.contains(STORE_NAME)) {
        db.createObjectStore(STORE_NAME);
      }
    };
  });

  return dbPromise;
}

async function getFromIndexedDB(key: string): Promise<string | null> {
  try {
    const db = await openDB();
    return new Promise((resolve) => {
      const tx = db.transaction(STORE_NAME, 'readonly');
      const store = tx.objectStore(STORE_NAME);
      const request = store.get(key);
      request.onsuccess = () => resolve(request.result ?? null);
      request.onerror = () => resolve(null);
    });
  } catch {
    return null;
  }
}

async function saveToIndexedDB(key: string, value: string): Promise<void> {
  try {
    const db = await openDB();
    const tx = db.transaction(STORE_NAME, 'readwrite');
    const store = tx.objectStore(STORE_NAME);
    store.put(value, key);
  } catch {
    // Silently fail - caching is optional
  }
}

function getCacheKey(assetId: string, modifiedTime?: number): string {
  return modifiedTime ? `${assetId}_${modifiedTime}` : assetId;
}

export async function getThumbnail(assetId: string, modifiedTime?: number): Promise<string | null> {
  const cacheKey = getCacheKey(assetId, modifiedTime);

  // L1: Check memory cache first (instant)
  if (memoryCache.has(cacheKey)) {
    return memoryCache.get(cacheKey) ?? null;
  }

  // If already fetching, wait for that promise
  if (pending.has(cacheKey)) {
    return pending.get(cacheKey)!;
  }

  // Start the caching pipeline
  const promise = (async () => {
    // L2: Check IndexedDB (fast, persistent)
    const cached = await getFromIndexedDB(cacheKey);
    if (cached) {
      memoryCache.set(cacheKey, cached);
      return cached;
    }

    // L3: Fetch from backend
    const result = await invoke<string | null>('get_thumbnail_base64', { assetId });

    // Store in both caches
    memoryCache.set(cacheKey, result);
    if (result) {
      saveToIndexedDB(cacheKey, result); // Fire and forget
    }

    return result;
  })();

  pending.set(cacheKey, promise);

  promise.finally(() => {
    pending.delete(cacheKey);
  });

  return promise;
}

// Preload thumbnails for upcoming assets (call with next page of assets)
export function preloadThumbnails(assets: Array<{ id: string; modified_time: number }>): void {
  // Limit concurrent preloads to avoid overwhelming the system
  const BATCH_SIZE = 10;
  const toPreload = assets.slice(0, BATCH_SIZE);

  toPreload.forEach(({ id, modified_time }) => {
    const cacheKey = getCacheKey(id, modified_time);
    // Only preload if not already cached or pending
    if (!memoryCache.has(cacheKey) && !pending.has(cacheKey)) {
      // Fire and forget - don't await
      getThumbnail(id, modified_time);
    }
  });
}

// Clear cache for an asset (e.g., after regeneration)
export function invalidateThumbnail(assetId: string, modifiedTime?: number): void {
  const cacheKey = getCacheKey(assetId, modifiedTime);
  memoryCache.delete(cacheKey);
}

// Clear all cache
export async function clearThumbnailCache(): Promise<void> {
  memoryCache.clear();
  try {
    const db = await openDB();
    const tx = db.transaction(STORE_NAME, 'readwrite');
    const store = tx.objectStore(STORE_NAME);
    store.clear();
  } catch {
    // Silently fail
  }
}
