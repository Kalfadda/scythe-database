import { useState, useEffect, memo } from 'react';
import { clsx } from 'clsx';
import type { Asset, AssetType } from '../types';
import { getThumbnail } from '../services/thumbnailCache';

interface AssetTileProps {
  asset: Asset;
  selected: boolean;
  onClick: () => void;
}

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

export const AssetTile = memo(function AssetTile({ asset, selected, onClick }: AssetTileProps) {
  const [imgSrc, setImgSrc] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    let cancelled = false;

    // Only try to load thumbnails for textures and materials
    if (asset.asset_type === 'texture' || asset.asset_type === 'material') {
      setLoading(true);

      // Use cached thumbnail fetch
      getThumbnail(asset.id)
        .then((data) => {
          if (!cancelled) {
            setImgSrc(data);
            setLoading(false);
          }
        });
    } else {
      setImgSrc(null);
    }

    return () => {
      cancelled = true;
    };
  }, [asset.id, asset.asset_type]);

  return (
    <div
      className={clsx('asset-tile', { selected })}
      onClick={onClick}
    >
      <div className="asset-thumbnail">
        {imgSrc ? (
          <img
            src={imgSrc}
            alt={asset.file_name}
            loading="lazy"
          />
        ) : loading ? (
          <div className="loading-spinner" style={{ width: 24, height: 24 }} />
        ) : (
          <span className="placeholder">{TYPE_ICONS[asset.asset_type]}</span>
        )}
      </div>
      <div className="asset-info">
        <div className="asset-name" title={asset.file_name}>
          {asset.file_name}
        </div>
        <span className={`type-badge ${asset.asset_type}`}>
          {asset.asset_type}
        </span>
      </div>
    </div>
  );
});
