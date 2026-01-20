import { open } from '@tauri-apps/plugin-dialog';
import { useStore } from '../state/store';

export function EmptyState() {
  const { setProjectRoot } = useStore();

  const handleSelectProject = async () => {
    const selected = await open({
      directory: true,
      title: 'Select Unity Project Root',
    });
    if (selected && typeof selected === 'string') {
      await setProjectRoot(selected);
    }
  };

  return (
    <div className="empty-state" style={{ flex: 1 }}>
      <div className="icon">🎮</div>
      <h2>Welcome to Scythe Database</h2>
      <p>Select a Unity project folder to get started</p>
      <button className="btn btn-primary" onClick={handleSelectProject}>
        Select Project Folder
      </button>
    </div>
  );
}
