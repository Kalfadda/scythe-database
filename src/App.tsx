import { useEffect, useRef } from 'react';
import { listen } from '@tauri-apps/api/event';
import { useStore } from './state/store';
import { Header } from './components/Header';
import { Sidebar } from './components/Sidebar';
import { AssetGrid } from './components/AssetGrid';
import { DetailPanel } from './components/DetailPanel';
import { ScanStatus } from './components/ScanStatus';
import { EmptyState } from './components/EmptyState';
import type { ScanProgress } from './types';

function App() {
  const { project, loadSettings, updateScanProgress, scanProgress, selectedAssetId, refreshAssets, loadTypeCounts } = useStore();
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

    return () => {
      unlistenProgress.then(fn => fn());
      unlistenAssets.then(fn => fn());
    };
  }, [loadSettings, updateScanProgress, refreshAssets, loadTypeCounts]);

  return (
    <div className="app">
      <Header />
      {scanProgress && <ScanStatus progress={scanProgress} />}
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
