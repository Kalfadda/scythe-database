import { useRef, useEffect, useCallback } from 'react';
import { useStore } from '../state/store';
import { AssetTile } from './AssetTile';

export function AssetGrid() {
  const { assets, totalCount, isLoading, loadMoreAssets, selectedAssetId, selectAsset } = useStore();
  const containerRef = useRef<HTMLDivElement>(null);
  const loadingRef = useRef(false);

  const handleScroll = useCallback(() => {
    if (!containerRef.current || loadingRef.current) return;

    const { scrollTop, scrollHeight, clientHeight } = containerRef.current;
    if (scrollHeight - scrollTop - clientHeight < 500) {
      loadingRef.current = true;
      loadMoreAssets().finally(() => {
        loadingRef.current = false;
      });
    }
  }, [loadMoreAssets]);

  useEffect(() => {
    const container = containerRef.current;
    if (container) {
      container.addEventListener('scroll', handleScroll);
      return () => container.removeEventListener('scroll', handleScroll);
    }
  }, [handleScroll]);

  if (assets.length === 0 && !isLoading) {
    return (
      <div className="grid-container">
        <div className="empty-state">
          <div className="icon">📁</div>
          <p>No assets found</p>
        </div>
      </div>
    );
  }

  return (
    <div className="grid-container" ref={containerRef}>
      {isLoading && assets.length > 0 && (
        <div className="grid-loading-overlay">
          <div className="loading-spinner" />
        </div>
      )}
      <div className="asset-grid">
        {assets.map((asset) => (
          <AssetTile
            key={asset.id}
            asset={asset}
            selected={asset.id === selectedAssetId}
            onClick={() => selectAsset(asset.id)}
          />
        ))}
      </div>
      {isLoading && assets.length === 0 && (
        <div style={{ display: 'flex', justifyContent: 'center', padding: '20px' }}>
          <div className="loading-spinner" />
        </div>
      )}
      {assets.length > 0 && !isLoading && (
        <div style={{ textAlign: 'center', padding: '10px', color: 'var(--text-secondary)' }}>
          Showing {assets.length} of {totalCount} assets
        </div>
      )}
    </div>
  );
}
