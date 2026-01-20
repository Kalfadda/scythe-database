import { useRef, useEffect, useCallback } from 'react';
import { useStore } from '../state/store';
import { AssetTile } from './AssetTile';
import { preloadThumbnails } from '../services/thumbnailCache';
import { preloadModelThumbnails } from '../services/modelThumbnailCache';

export function AssetGrid() {
  const assets = useStore((s) => s.assets);
  const totalCount = useStore((s) => s.totalCount);
  const isLoading = useStore((s) => s.isLoading);
  const loadMoreAssets = useStore((s) => s.loadMoreAssets);
  const selectedAssetId = useStore((s) => s.selectedAssetId);
  const selectAsset = useStore((s) => s.selectAsset);

  const containerRef = useRef<HTMLDivElement>(null);

  // Check if we need to load more (content doesn't fill viewport or near bottom)
  const checkLoadMore = useCallback(() => {
    const container = containerRef.current;
    if (!container) return;

    const { scrollTop, scrollHeight, clientHeight } = container;
    // Load more if content doesn't fill container OR within 500px of bottom
    if (scrollHeight <= clientHeight || scrollHeight - scrollTop - clientHeight < 500) {
      loadMoreAssets();
    }
  }, [loadMoreAssets]);

  // Scroll listener
  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    container.addEventListener('scroll', checkLoadMore, { passive: true });
    return () => container.removeEventListener('scroll', checkLoadMore);
  }, [checkLoadMore]);

  // Check on resize (window maximize/restore)
  useEffect(() => {
    const handleResize = () => {
      // Small delay to let layout settle
      setTimeout(checkLoadMore, 100);
    };

    window.addEventListener('resize', handleResize);
    return () => window.removeEventListener('resize', handleResize);
  }, [checkLoadMore]);

  // Check after assets load (in case content doesn't fill viewport)
  useEffect(() => {
    if (assets.length > 0 && !isLoading) {
      // Small delay to let DOM update
      setTimeout(checkLoadMore, 50);
    }
  }, [assets.length, isLoading, checkLoadMore]);

  // Preload thumbnails for visible and upcoming assets
  useEffect(() => {
    if (assets.length === 0) return;

    // Preload thumbnails for textures/materials
    const textureAssets = assets
      .filter(a => a.asset_type === 'texture' || a.asset_type === 'material')
      .map(a => ({ id: a.id, modified_time: a.modified_time }));
    preloadThumbnails(textureAssets);

    // Preload thumbnails for models
    const modelAssets = assets
      .filter(a => a.asset_type === 'model')
      .map(a => ({
        id: a.id,
        absolute_path: a.absolute_path,
        extension: a.extension,
        modified_time: a.modified_time
      }));
    preloadModelThumbnails(modelAssets);
  }, [assets]);

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
