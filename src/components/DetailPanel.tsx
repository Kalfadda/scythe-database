import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useStore } from '../state/store';
import { ModelPreview } from './ModelPreview';
import type { AssetType, MaterialInfo, ModelInfo, BundlePreview } from '../types';

const TYPE_ICONS: Record<AssetType, string> = {
  texture: '🖼️',
  model: '📦',
  material: '🎨',
  prefab: '🧩',
  audio: '🔊',
  shader: '✨',
  scene: '🎬',
  scriptable_object: '📜',
  unknown: '📄',
};

function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
}

function formatDate(timestamp: number): string {
  return new Date(timestamp * 1000).toLocaleString();
}

function formatNumber(n: number): string {
  return n.toLocaleString();
}

export function DetailPanel() {
  const {
    selectedAsset,
    dependencies,
    dependents,
    selectAsset,
    exportFile,
    exportBundle,
    outputFolder,
    assets
  } = useStore();

  const [materialInfo, setMaterialInfo] = useState<MaterialInfo | null>(null);
  const [modelInfo, setModelInfo] = useState<ModelInfo | null>(null);
  const [bundlePreview, setBundlePreview] = useState<BundlePreview | null>(null);
  const [imgSrc, setImgSrc] = useState<string | null>(null);
  const [showBundleDetails, setShowBundleDetails] = useState(false);

  useEffect(() => {
    if (!selectedAsset) {
      setMaterialInfo(null);
      setModelInfo(null);
      setBundlePreview(null);
      setImgSrc(null);
      return;
    }

    // Load thumbnail via base64
    if (selectedAsset.asset_type === 'texture' || selectedAsset.asset_type === 'material') {
      invoke<string | null>('get_thumbnail_base64', { assetId: selectedAsset.id })
        .then(setImgSrc)
        .catch(() => setImgSrc(null));
    } else {
      setImgSrc(null);
    }

    // Load material info
    if (selectedAsset.asset_type === 'material') {
      invoke<MaterialInfo | null>('get_material_info', { assetId: selectedAsset.id })
        .then(setMaterialInfo)
        .catch(console.error);
    } else {
      setMaterialInfo(null);
    }

    // Load model info
    if (selectedAsset.asset_type === 'model') {
      invoke<ModelInfo | null>('get_model_info', { assetId: selectedAsset.id })
        .then(setModelInfo)
        .catch(console.error);
    } else {
      setModelInfo(null);
    }

    // Load bundle preview
    invoke<BundlePreview>('get_bundle_preview', { assetId: selectedAsset.id })
      .then(setBundlePreview)
      .catch(() => setBundlePreview(null));

  }, [selectedAsset?.id]);

  if (!selectedAsset) return null;

  const handleRevealInExplorer = async () => {
    await invoke('reveal_in_explorer', { path: selectedAsset.absolute_path });
  };

  const handleCopyPath = async () => {
    await navigator.clipboard.writeText(selectedAsset.absolute_path);
  };

  const handleExportFile = async () => {
    if (!outputFolder) {
      alert('Please select an output folder first');
      return;
    }
    await exportFile(selectedAsset.id);
  };

  const handleExportBundle = async () => {
    if (!outputFolder) {
      alert('Please select an output folder first');
      return;
    }
    await exportBundle(selectedAsset.id);
  };

  const navigateToDependency = (assetId: string | null) => {
    if (assetId) {
      selectAsset(assetId);
    }
  };

  const getAssetName = (assetId: string | null) => {
    if (!assetId) return 'Unresolved';
    const asset = assets.find(a => a.id === assetId);
    return asset?.file_name ?? 'Unknown';
  };

  return (
    <aside className="detail-panel">
      <div className="detail-header">
        <button
          className="btn btn-secondary"
          style={{ float: 'right' }}
          onClick={() => selectAsset(null)}
        >
          Close
        </button>
        <h2 style={{ fontSize: '16px', marginBottom: '8px' }}>{selectedAsset.file_name}</h2>
        <span className={`type-badge ${selectedAsset.asset_type}`}>
          {selectedAsset.asset_type}
        </span>
      </div>

      <div className="detail-preview">
        {selectedAsset.asset_type === 'model' ? (
          <ModelPreview
            filePath={selectedAsset.absolute_path}
            extension={selectedAsset.extension}
          />
        ) : imgSrc ? (
          <img
            src={imgSrc}
            alt={selectedAsset.file_name}
          />
        ) : (
          <span style={{ fontSize: '64px' }}>{TYPE_ICONS[selectedAsset.asset_type]}</span>
        )}
      </div>

      <div className="detail-section">
        <h3>File Info</h3>
        <div className="detail-row">
          <span className="label">Path</span>
          <span className="value" title={selectedAsset.relative_path}>
            {selectedAsset.relative_path}
          </span>
        </div>
        <div className="detail-row">
          <span className="label">Size</span>
          <span className="value">{formatBytes(selectedAsset.size_bytes)}</span>
        </div>
        <div className="detail-row">
          <span className="label">Modified</span>
          <span className="value">{formatDate(selectedAsset.modified_time)}</span>
        </div>
        {selectedAsset.unity_guid && (
          <div className="detail-row">
            <span className="label">GUID</span>
            <span className="value" style={{ fontSize: '11px', fontFamily: 'monospace' }}>
              {selectedAsset.unity_guid}
            </span>
          </div>
        )}
      </div>

      {/* Model Stats */}
      {modelInfo && (
        <div className="detail-section">
          <h3>Model Stats</h3>
          {modelInfo.vertex_count !== null && (
            <div className="detail-row">
              <span className="label">Vertices</span>
              <span className="value">{formatNumber(modelInfo.vertex_count)}</span>
            </div>
          )}
          {modelInfo.triangle_count !== null && (
            <div className="detail-row">
              <span className="label">Triangles</span>
              <span className="value">{formatNumber(modelInfo.triangle_count)}</span>
            </div>
          )}
          {modelInfo.submesh_count !== null && (
            <div className="detail-row">
              <span className="label">Submeshes</span>
              <span className="value">{modelInfo.submesh_count}</span>
            </div>
          )}
          <div className="detail-row">
            <span className="label">Normals</span>
            <span className="value">{modelInfo.has_normals ? 'Yes' : 'No'}</span>
          </div>
          <div className="detail-row">
            <span className="label">UVs</span>
            <span className="value">{modelInfo.has_uvs ? 'Yes' : 'No'}</span>
          </div>
        </div>
      )}

      {/* Material Info */}
      {materialInfo && (
        <div className="detail-section">
          <h3>Material Textures</h3>
          {materialInfo.shader_name && (
            <div className="detail-row" style={{ marginBottom: '12px' }}>
              <span className="label">Shader</span>
              <span className="value">{materialInfo.shader_name}</span>
            </div>
          )}
          {materialInfo.textures.length > 0 ? (
            <div className="material-textures">
              {materialInfo.textures.map((tex, i) => (
                <div key={i} className="material-texture-slot">
                  <span className="slot-name">{tex.slot_name.replace('_', '')}</span>
                  {tex.texture_guid ? (
                    <span
                      className="slot-value clickable"
                      onClick={() => {
                        const texAsset = assets.find(a => a.unity_guid === tex.texture_guid);
                        if (texAsset) selectAsset(texAsset.id);
                      }}
                    >
                      {tex.texture_guid.substring(0, 8)}...
                    </span>
                  ) : (
                    <span className="slot-value empty">None</span>
                  )}
                </div>
              ))}
            </div>
          ) : (
            <p style={{ color: 'var(--text-secondary)', fontSize: '12px' }}>No textures found</p>
          )}
        </div>
      )}

      {/* Bundle Preview */}
      {bundlePreview && bundlePreview.dependencies.length > 0 && (
        <div className="detail-section">
          <h3
            style={{ cursor: 'pointer', display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}
            onClick={() => setShowBundleDetails(!showBundleDetails)}
          >
            <span>Bundle Contents ({bundlePreview.dependencies.length + 1} files)</span>
            <span style={{ fontSize: '10px' }}>{showBundleDetails ? '▼' : '▶'}</span>
          </h3>
          <div className="detail-row">
            <span className="label">Total Size</span>
            <span className="value">{formatBytes(bundlePreview.total_size_bytes)}</span>
          </div>
          {showBundleDetails && (
            <div className="bundle-contents">
              <div className="bundle-item root">
                <span className="bundle-icon">{TYPE_ICONS[bundlePreview.root_asset.asset_type as AssetType] || '📄'}</span>
                <span className="bundle-name">{bundlePreview.root_asset.file_name}</span>
                <span className="bundle-size">{formatBytes(bundlePreview.root_asset.size_bytes)}</span>
              </div>
              {bundlePreview.dependencies.map((dep) => (
                <div
                  key={dep.id}
                  className="bundle-item"
                  onClick={() => selectAsset(dep.id)}
                >
                  <span className="bundle-icon">{TYPE_ICONS[dep.asset_type as AssetType] || '📄'}</span>
                  <span className="bundle-name">{dep.file_name}</span>
                  <span className="bundle-size">{formatBytes(dep.size_bytes)}</span>
                </div>
              ))}
            </div>
          )}
        </div>
      )}

      {dependencies.length > 0 && (
        <div className="detail-section">
          <h3>Dependencies ({dependencies.length})</h3>
          <div className="dependency-list">
            {dependencies.map((dep) => (
              <div
                key={dep.id}
                className="dependency-item"
                onClick={() => navigateToDependency(dep.to_asset_id)}
                style={{ opacity: dep.to_asset_id ? 1 : 0.5 }}
              >
                {getAssetName(dep.to_asset_id)}
                {!dep.to_asset_id && ` (${dep.to_guid.substring(0, 8)}...)`}
              </div>
            ))}
          </div>
        </div>
      )}

      {dependents.length > 0 && (
        <div className="detail-section">
          <h3>Used By ({dependents.length})</h3>
          <div className="dependency-list">
            {dependents.map((dep) => (
              <div
                key={dep.id}
                className="dependency-item"
                onClick={() => navigateToDependency(dep.from_asset_id)}
              >
                {getAssetName(dep.from_asset_id)}
              </div>
            ))}
          </div>
        </div>
      )}

      <div className="detail-actions">
        <button className="btn btn-secondary" onClick={handleExportFile}>
          Export File Only
        </button>
        <button
          className="btn btn-primary"
          onClick={handleExportBundle}
          title={bundlePreview ? `Export ${bundlePreview.dependencies.length + 1} files (${formatBytes(bundlePreview.total_size_bytes)})` : 'Export bundle'}
        >
          Export Bundle{bundlePreview && bundlePreview.dependencies.length > 0
            ? ` (${bundlePreview.dependencies.length + 1} files)`
            : ''}
        </button>
        <button className="btn btn-secondary" onClick={handleRevealInExplorer}>
          Reveal in Explorer
        </button>
        <button className="btn btn-secondary" onClick={handleCopyPath}>
          Copy Path
        </button>
      </div>
    </aside>
  );
}
