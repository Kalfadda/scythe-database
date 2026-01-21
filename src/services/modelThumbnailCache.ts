import * as THREE from 'three';
import { OBJLoader } from 'three/examples/jsm/loaders/OBJLoader.js';
import { FBXLoader } from 'three/examples/jsm/loaders/FBXLoader.js';
import { GLTFLoader } from 'three/examples/jsm/loaders/GLTFLoader.js';
import { convertFileSrc } from '@tauri-apps/api/core';

// L1: In-memory cache for instant retrieval within session
const memoryCache = new Map<string, string | null>();
const pending = new Map<string, Promise<string | null>>();

// L2: IndexedDB for persistent storage across sessions
const DB_NAME = 'ModelThumbnailCache';
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

function getCacheKey(assetId: string, modifiedTime: number): string {
  return `${assetId}_${modifiedTime}`;
}

// Shared renderer for generating thumbnails
let renderer: THREE.WebGLRenderer | null = null;
let scene: THREE.Scene | null = null;
let camera: THREE.PerspectiveCamera | null = null;

function initRenderer() {
  if (renderer) return;

  renderer = new THREE.WebGLRenderer({
    antialias: true,
    alpha: true,
    preserveDrawingBuffer: true
  });
  renderer.setSize(256, 256);
  renderer.setClearColor(0x1a1a2e, 1);

  scene = new THREE.Scene();

  // Lighting
  const ambientLight = new THREE.AmbientLight(0xffffff, 0.5);
  scene.add(ambientLight);

  const directionalLight = new THREE.DirectionalLight(0xffffff, 0.8);
  directionalLight.position.set(5, 5, 5);
  scene.add(directionalLight);

  const backLight = new THREE.DirectionalLight(0xffffff, 0.3);
  backLight.position.set(-5, 3, -5);
  scene.add(backLight);

  camera = new THREE.PerspectiveCamera(45, 1, 0.1, 1000);
}

async function loadModel(filePath: string, extension: string): Promise<THREE.Object3D | null> {
  const fileUrl = convertFileSrc(filePath);

  return new Promise((resolve) => {
    let loader: OBJLoader | FBXLoader | GLTFLoader;

    switch (extension.toLowerCase()) {
      case 'obj':
        loader = new OBJLoader();
        break;
      case 'fbx':
        loader = new FBXLoader();
        break;
      case 'gltf':
      case 'glb':
        loader = new GLTFLoader();
        break;
      default:
        resolve(null);
        return;
    }

    loader.load(
      fileUrl,
      (result) => {
        let object: THREE.Object3D;
        if ('scene' in result) {
          object = result.scene;
        } else {
          object = result;
        }

        // Ensure materials are visible
        object.traverse((child) => {
          if (child instanceof THREE.Mesh) {
            if (!child.material || (Array.isArray(child.material) && child.material.length === 0)) {
              child.material = new THREE.MeshStandardMaterial({
                color: 0x888888,
                metalness: 0.3,
                roughness: 0.7,
              });
            }
          }
        });

        resolve(object);
      },
      undefined,
      () => resolve(null)
    );
  });
}

async function renderModelToThumbnail(filePath: string, extension: string): Promise<string | null> {
  try {
    initRenderer();
    if (!renderer || !scene || !camera) return null;

    const model = await loadModel(filePath, extension);
    if (!model) return null;

    // Clear previous model from scene (keep lights)
    const toRemove: THREE.Object3D[] = [];
    scene.traverse((child) => {
      if (child instanceof THREE.Group || child instanceof THREE.Mesh) {
        if (child.parent === scene) {
          toRemove.push(child);
        }
      }
    });
    toRemove.forEach(obj => scene!.remove(obj));

    // Auto-scale and center
    const box = new THREE.Box3().setFromObject(model);
    const size = box.getSize(new THREE.Vector3());
    const center = box.getCenter(new THREE.Vector3());

    const maxDim = Math.max(size.x, size.y, size.z);
    const scale = 2 / maxDim;
    model.scale.multiplyScalar(scale);

    // Recalculate after scaling
    box.setFromObject(model);
    box.getCenter(center);
    model.position.sub(center);

    scene.add(model);

    // Position camera for a nice 3/4 view
    camera.position.set(2.5, 2, 2.5);
    camera.lookAt(0, 0, 0);

    // Render
    renderer.render(scene, camera);

    // Get data URL
    const dataUrl = renderer.domElement.toDataURL('image/png');

    // Cleanup
    scene.remove(model);
    model.traverse((child) => {
      if (child instanceof THREE.Mesh) {
        child.geometry?.dispose();
        if (child.material) {
          if (Array.isArray(child.material)) {
            child.material.forEach(m => m.dispose());
          } else {
            child.material.dispose();
          }
        }
      }
    });

    return dataUrl;
  } catch (e) {
    console.error('Failed to render model thumbnail:', e);
    return null;
  }
}

export async function getModelThumbnail(
  assetId: string,
  filePath: string,
  extension: string,
  modifiedTime?: number
): Promise<string | null> {
  const cacheKey = modifiedTime ? getCacheKey(assetId, modifiedTime) : assetId;

  // L1: Check memory cache first (instant)
  if (memoryCache.has(cacheKey)) {
    return memoryCache.get(cacheKey) ?? null;
  }

  // If already rendering, wait for that promise
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

    // L3: Render with Three.js (slow, but only once per asset)
    const rendered = await renderModelToThumbnail(filePath, extension);

    // Store in both caches
    memoryCache.set(cacheKey, rendered);
    if (rendered) {
      saveToIndexedDB(cacheKey, rendered); // Fire and forget
    }

    return rendered;
  })();

  pending.set(cacheKey, promise);

  promise.finally(() => {
    pending.delete(cacheKey);
  });

  return promise;
}

export function invalidateModelThumbnail(assetId: string, modifiedTime?: number): void {
  const cacheKey = modifiedTime ? getCacheKey(assetId, modifiedTime) : assetId;
  memoryCache.delete(cacheKey);
  // Note: IndexedDB entry will be stale but ignored due to different modifiedTime key
}

export async function clearModelThumbnailCache(): Promise<void> {
  memoryCache.clear();
  // Also clear IndexedDB
  try {
    const db = await openDB();
    const tx = db.transaction(STORE_NAME, 'readwrite');
    const store = tx.objectStore(STORE_NAME);
    store.clear();
  } catch {
    // Silently fail
  }
}

// Preload model thumbnails for upcoming assets
export function preloadModelThumbnails(
  assets: Array<{ id: string; absolute_path: string; extension: string; modified_time: number }>
): void {
  // Limit concurrent preloads to avoid overwhelming the system (models are heavier)
  const BATCH_SIZE = 5;
  const toPreload = assets.slice(0, BATCH_SIZE);

  toPreload.forEach(({ id, absolute_path, extension, modified_time }) => {
    const cacheKey = getCacheKey(id, modified_time);
    // Only preload if not already cached or pending
    if (!memoryCache.has(cacheKey) && !pending.has(cacheKey)) {
      // Fire and forget - don't await
      getModelThumbnail(id, absolute_path, extension, modified_time);
    }
  });
}

// Generate all model thumbnails with progress reporting
export async function generateAllModelThumbnails(
  assets: Array<{ id: string; absolute_path: string; extension: string; modified_time: number }>,
  onProgress: (generated: number, total: number) => void
): Promise<void> {
  const total = assets.length;
  let generated = 0;

  // Clear IndexedDB cache first to force regeneration
  await clearModelThumbnailCache();

  for (const asset of assets) {
    try {
      // Force render (cache was cleared)
      await renderModelToThumbnail(asset.absolute_path, asset.extension).then((result) => {
        if (result) {
          const cacheKey = getCacheKey(asset.id, asset.modified_time);
          memoryCache.set(cacheKey, result);
          saveToIndexedDB(cacheKey, result);
        }
      });
    } catch (e) {
      console.error(`Failed to generate thumbnail for ${asset.id}:`, e);
    }

    generated++;
    onProgress(generated, total);
  }
}
