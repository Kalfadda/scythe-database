import { useState, useCallback, useEffect, useRef } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import { useStore } from '../state/store';

export function Header() {
  const { projectRoot, outputFolder, setProjectRoot, setOutputFolder, searchQuery, search, startScan, project } = useStore();
  const [localSearch, setLocalSearch] = useState(searchQuery);
  const debounceRef = useRef<ReturnType<typeof setTimeout>>();

  const handleSelectProject = useCallback(async () => {
    const selected = await open({
      directory: true,
      title: 'Select Unity Project Root',
    });
    if (selected && typeof selected === 'string') {
      await setProjectRoot(selected);
    }
  }, [setProjectRoot]);

  const handleSelectOutput = useCallback(async () => {
    const selected = await open({
      directory: true,
      title: 'Select Export Output Folder',
    });
    if (selected && typeof selected === 'string') {
      await setOutputFolder(selected);
    }
  }, [setOutputFolder]);

  // Debounce search
  useEffect(() => {
    if (debounceRef.current) {
      clearTimeout(debounceRef.current);
    }
    debounceRef.current = setTimeout(() => {
      if (localSearch !== searchQuery) {
        search(localSearch);
      }
    }, 300);
    return () => {
      if (debounceRef.current) {
        clearTimeout(debounceRef.current);
      }
    };
  }, [localSearch, search, searchQuery]);

  const handleSearch = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    setLocalSearch(e.target.value);
  }, []);

  const handleRefresh = useCallback(() => {
    if (project) {
      startScan();
    }
  }, [project, startScan]);

  return (
    <header className="header">
      <h1>Scythe Database</h1>

      <button className="btn btn-secondary" onClick={handleSelectProject}>
        {projectRoot ? 'Change Project' : 'Select Project'}
      </button>

      {projectRoot && (
        <div className="project-path" title={projectRoot}>
          {projectRoot.split(/[\\/]/).pop()}
        </div>
      )}

      {project && (
        <>
          <input
            type="text"
            className="input search-input"
            placeholder="Search assets..."
            value={localSearch}
            onChange={handleSearch}
          />

          <button className="btn btn-secondary" onClick={handleRefresh}>
            Refresh
          </button>
        </>
      )}

      <div style={{ flex: 1 }} />

      <button className="btn btn-secondary" onClick={handleSelectOutput}>
        {outputFolder ? 'Output: ' + outputFolder.split(/[\\/]/).pop() : 'Set Output Folder'}
      </button>
    </header>
  );
}
