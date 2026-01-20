import { Suspense, useRef, useState, useEffect } from 'react';
import { Canvas, useFrame } from '@react-three/fiber';
import { OrbitControls, Center, Grid } from '@react-three/drei';
import { convertFileSrc } from '@tauri-apps/api/core';
import * as THREE from 'three';
import { OBJLoader } from 'three/examples/jsm/loaders/OBJLoader.js';
import { FBXLoader } from 'three/examples/jsm/loaders/FBXLoader.js';
import { GLTFLoader } from 'three/examples/jsm/loaders/GLTFLoader.js';

interface ModelPreviewProps {
  filePath: string;
  extension: string;
}

interface ModelStats {
  vertices: number;
  triangles: number;
}

function Model({ filePath, extension, onStats }: ModelPreviewProps & { onStats: (stats: ModelStats) => void }) {
  const [model, setModel] = useState<THREE.Object3D | null>(null);
  const [error, setError] = useState<string | null>(null);
  const groupRef = useRef<THREE.Group>(null);

  useEffect(() => {
    let cancelled = false;

    const loadModel = async () => {
      try {
        // Convert local file path to URL accessible by webview
        const fileUrl = convertFileSrc(filePath);

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
            setError(`Unsupported format: ${extension}`);
            return;
        }

        loader.load(
          fileUrl,
          (result) => {
            if (cancelled) return;

            let object: THREE.Object3D;
            if ('scene' in result) {
              // GLTF
              object = result.scene;
            } else {
              object = result;
            }

            // Calculate stats
            let vertices = 0;
            let triangles = 0;

            object.traverse((child) => {
              if (child instanceof THREE.Mesh) {
                const geometry = child.geometry;
                if (geometry.index) {
                  triangles += geometry.index.count / 3;
                } else if (geometry.attributes.position) {
                  triangles += geometry.attributes.position.count / 3;
                }
                if (geometry.attributes.position) {
                  vertices += geometry.attributes.position.count;
                }

                // Ensure material is visible
                if (!child.material) {
                  child.material = new THREE.MeshStandardMaterial({
                    color: 0x888888,
                    metalness: 0.3,
                    roughness: 0.7,
                  });
                }
              }
            });

            onStats({ vertices, triangles: Math.floor(triangles) });

            // Auto-scale and center
            const box = new THREE.Box3().setFromObject(object);
            const size = box.getSize(new THREE.Vector3());
            const maxDim = Math.max(size.x, size.y, size.z);
            const scale = 2 / maxDim;
            object.scale.multiplyScalar(scale);

            // Center the model
            box.setFromObject(object);
            const center = box.getCenter(new THREE.Vector3());
            object.position.sub(center);

            setModel(object);
          },
          undefined,
          (err) => {
            if (!cancelled) {
              console.error('Model load error:', err);
              setError('Failed to load model');
            }
          }
        );
      } catch (e) {
        if (!cancelled) {
          setError('Error loading model');
        }
      }
    };

    loadModel();
    return () => { cancelled = true; };
  }, [filePath, extension, onStats]);

  // Slow rotation
  useFrame((_, delta) => {
    if (groupRef.current && model) {
      groupRef.current.rotation.y += delta * 0.3;
    }
  });

  if (error) {
    return null;
  }

  if (!model) {
    return null;
  }

  return (
    <group ref={groupRef}>
      <primitive object={model} />
    </group>
  );
}

function Fallback() {
  return (
    <mesh>
      <boxGeometry args={[1, 1, 1]} />
      <meshStandardMaterial color="#666" wireframe />
    </mesh>
  );
}

export function ModelPreview({ filePath, extension }: ModelPreviewProps) {
  const [stats, setStats] = useState<ModelStats | null>(null);
  const [wireframe, setWireframe] = useState(false);

  return (
    <div className="model-preview-container">
      <Canvas
        camera={{ position: [3, 2, 3], fov: 45 }}
        style={{ background: 'linear-gradient(180deg, #1a1a2e 0%, #0f0f23 100%)' }}
      >
        <ambientLight intensity={0.4} />
        <directionalLight position={[5, 5, 5]} intensity={0.8} />
        <directionalLight position={[-5, 3, -5]} intensity={0.3} />

        <Suspense fallback={<Fallback />}>
          <Center>
            <Model filePath={filePath} extension={extension} onStats={setStats} />
          </Center>
        </Suspense>

        <OrbitControls
          enablePan={false}
          minDistance={1}
          maxDistance={10}
        />

        <Grid
          position={[0, -1.5, 0]}
          args={[10, 10]}
          cellSize={0.5}
          cellThickness={0.5}
          cellColor="#333"
          sectionSize={2}
          sectionThickness={1}
          sectionColor="#444"
          fadeDistance={10}
          fadeStrength={1}
        />
      </Canvas>

      {stats && (
        <div className="model-stats-overlay">
          <span>{stats.vertices.toLocaleString()} verts</span>
          <span>{stats.triangles.toLocaleString()} tris</span>
        </div>
      )}

      <div className="model-controls">
        <button
          className="btn btn-secondary model-control-btn"
          onClick={() => setWireframe(!wireframe)}
          title="Toggle wireframe"
        >
          {wireframe ? '◼' : '◻'}
        </button>
      </div>
    </div>
  );
}
