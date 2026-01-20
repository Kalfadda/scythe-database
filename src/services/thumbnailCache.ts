import { invoke } from '@tauri-apps/api/core';

// Simple in-memory cache for thumbnails
const cache = new Map<string, string | null>();
const pending = new Map<string, Promise<string | null>>();

export async function getThumbnail(assetId: string): Promise<string | null> {
  // Return from cache if available
  if (cache.has(assetId)) {
    return cache.get(assetId) ?? null;
  }

  // If already fetching, wait for that promise
  if (pending.has(assetId)) {
    return pending.get(assetId)!;
  }

  // Start fetch
  const promise = invoke<string | null>('get_thumbnail_base64', { assetId })
    .then((result) => {
      cache.set(assetId, result);
      pending.delete(assetId);
      return result;
    })
    .catch(() => {
      cache.set(assetId, null);
      pending.delete(assetId);
      return null;
    });

  pending.set(assetId, promise);
  return promise;
}

// Clear cache for an asset (e.g., after regeneration)
export function invalidateThumbnail(assetId: string): void {
  cache.delete(assetId);
}

// Clear all cache
export function clearThumbnailCache(): void {
  cache.clear();
}
