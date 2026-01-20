import { useRef, useEffect, useCallback } from 'react';
import { useStore } from '../state/store';
import { AssetTile } from './AssetTile';

export function AssetGrid() {
  const assets = useStore((s) => s.assets);
  const totalCount = useStore((s) => s.totalCount);
  const isLoading = useStore((s) => s.isLoading);
  const loadMoreAssets = useStore((s) => s.loadMoreAssets);
  const selectedAssetId = useStore((s) => s.selectedAssetId);
  const selectAsset = useStore((s) => s.selectAsset);

  const containerRef = useRef<HTMLDivElement>(null);

  const handleScroll = useCallback(() => {
    const container = containerRef.current;
    if (!container) return;

    const { scrollTop, scrollHeight, clientHeight } = container;
    // Load more when within 500px of bottom
    if (scrollHeight - scrollTop - clientHeight < 500) {
      loadMoreAssets();
    }
  }, [loadMoreAssets]);

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    // Use passive listener for better scroll performance
    container.addEventListener('scroll', handleScroll, { passive: true });
    return () => container.removeEventListener('scroll', handleScroll);
  }, [handleScroll]);

  const hasMore = assets.length < totalCount;

  return (
    <div className="grid-container" ref={containerRef}>
      {/* Loading overlay - only show when reloading from scratch */}
      {isLoading && assets.length > 0 && (
        <div className="grid-loading-overlay">
          <div className="loading-spinner" />
        </div>
      )}

      {/* Empty state */}
      {assets.length === 0 && !isLoading && (
        <div className="empty-state">
          <div className="icon">📁</div>
          <p>No assets found</p>
        </div>
      )}

      {/* Initial loading */}
      {assets.length === 0 && isLoading && (
        <div className="empty-state">
          <div className="loading-spinner" />
          <p>Loading assets...</p>
        </div>
      )}

      {/* Asset grid */}
      {assets.length > 0 && (
        <>
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

          {/* Footer status */}
          <div style={{ textAlign: 'center', padding: '16px', color: 'var(--text-secondary)' }}>
            {isLoading && hasMore ? (
              <div className="loading-spinner" style={{ margin: '0 auto' }} />
            ) : hasMore ? (
              <span>Showing {assets.length} of {totalCount} - scroll for more</span>
            ) : (
              <span>Showing all {assets.length} assets</span>
            )}
          </div>
        </>
      )}
    </div>
  );
}
