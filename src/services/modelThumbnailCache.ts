import * as THREE from 'three';
import { OBJLoader } from 'three/examples/jsm/loaders/OBJLoader.js';
import { FBXLoader } from 'three/examples/jsm/loaders/FBXLoader.js';
import { GLTFLoader } from 'three/examples/jsm/loaders/GLTFLoader.js';
import { convertFileSrc } from '@tauri-apps/api/core';

const cache = new Map<string, string | null>();
const pending = new Map<string, Promise<string | null>>();

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

export async function getModelThumbnail(assetId: string, filePath: string, extension: string): Promise<string | null> {
  // Return from cache if available
  if (cache.has(assetId)) {
    return cache.get(assetId) ?? null;
  }

  // If already rendering, wait for that promise
  if (pending.has(assetId)) {
    return pending.get(assetId)!;
  }

  // Start render
  const promise = renderModelToThumbnail(filePath, extension)
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

export function invalidateModelThumbnail(assetId: string): void {
  cache.delete(assetId);
}

export function clearModelThumbnailCache(): void {
  cache.clear();
}
