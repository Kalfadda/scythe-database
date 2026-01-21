import { useEffect, useRef } from 'react';
import { listen } from '@tauri-apps/api/event';
import { useStore } from './state/store';
import { Header } from './components/Header';
import { Sidebar } from './components/Sidebar';
import { AssetGrid } from './components/AssetGrid';
import { DetailPanel } from './components/DetailPanel';
import { ScanStatus } from './components/ScanStatus';
import { EmptyState } from './components/EmptyState';
import type { ScanProgress, ThumbnailProgress } from './types';

function App() {
  const { project, loadSettings, updateScanProgress, updateThumbnailProgress, scanProgress, thumbnailProgress, selectedAssetId, refreshAssets, loadTypeCounts } = useStore();
  const refreshPending = useRef(false);

  useEffect(() => {
    loadSettings();

    const unlistenProgress = listen<ScanProgress>('scan-progress', (event) => {
      updateScanProgress(event.payload);
    });

    // Listen for asset updates and refresh the grid
    const unlistenAssets = listen<number>('assets-updated', () => {
      // Debounce refreshes
      if (!refreshPending.current) {
        refreshPending.current = true;
        setTimeout(() => {
          refreshAssets();
          loadTypeCounts();
          refreshPending.current = false;
        }, 100);
      }
    });

    // Listen for thumbnail generation progress
    const unlistenThumbnails = listen<ThumbnailProgress>('thumbnail-progress', (event) => {
      updateThumbnailProgress(event.payload);
    });

    return () => {
      unlistenProgress.then(fn => fn());
      unlistenAssets.then(fn => fn());
      unlistenThumbnails.then(fn => fn());
    };
  }, [loadSettings, updateScanProgress, updateThumbnailProgress, refreshAssets, loadTypeCounts]);

  return (
    <div className="app">
      <Header />
      {scanProgress && <ScanStatus progress={scanProgress} />}
      {thumbnailProgress && (
        <div className="scan-status">
          <div className="loading-spinner" />
          <div className="text">
            {thumbnailProgress.phase === 'counting' && 'Counting assets...'}
            {thumbnailProgress.phase === 'generating' && `Generating texture thumbnails... (${thumbnailProgress.generated}/${thumbnailProgress.total})`}
            {thumbnailProgress.phase === 'generating_models' && `Generating model thumbnails... (${thumbnailProgress.generated}/${thumbnailProgress.total})`}
          </div>
          {thumbnailProgress.total > 0 && (
            <div className="progress-bar" style={{ flex: 1, maxWidth: '200px' }}>
              <div className="fill" style={{ width: `${Math.round((thumbnailProgress.generated / thumbnailProgress.total) * 100)}%` }} />
            </div>
          )}
        </div>
      )}
      <div className="main-content">
        {project ? (
          <>
            <Sidebar />
            <div className="content-area">
              <AssetGrid />
            </div>
            {selectedAssetId && <DetailPanel />}
          </>
        ) : (
          <EmptyState />
        )}
      </div>
    </div>
  );
}

export default App;
