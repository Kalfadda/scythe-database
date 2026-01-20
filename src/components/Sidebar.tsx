import { useStore } from '../state/store';
import type { AssetType } from '../types';

const ASSET_TYPES: { type: AssetType; label: string }[] = [
  { type: 'texture', label: 'Textures' },
  { type: 'model', label: 'Models' },
  { type: 'material', label: 'Materials' },
  { type: 'prefab', label: 'Prefabs' },
  { type: 'audio', label: 'Audio' },
  { type: 'shader', label: 'Shaders' },
  { type: 'scene', label: 'Scenes' },
  { type: 'scriptable_object', label: 'ScriptableObjects' },
];

export function Sidebar() {
  const { selectedTypes, toggleTypeFilter, typeCounts, totalCount } = useStore();

  const getCount = (type: AssetType) => {
    const found = typeCounts.find(tc => tc.asset_type === type);
    return found?.count ?? 0;
  };

  return (
    <aside className="sidebar">
      <div className="filter-section">
        <h3>Asset Types</h3>
        <div className="filter-item">
          <input
            type="checkbox"
            id="filter-all"
            checked={selectedTypes.length === 0}
            onChange={() => {
              if (selectedTypes.length > 0) {
                selectedTypes.forEach(t => toggleTypeFilter(t));
              }
            }}
          />
          <label htmlFor="filter-all">All</label>
          <span className="count">{totalCount}</span>
        </div>
        {ASSET_TYPES.map(({ type, label }) => (
          <div key={type} className="filter-item">
            <input
              type="checkbox"
              id={`filter-${type}`}
              checked={selectedTypes.includes(type)}
              onChange={() => toggleTypeFilter(type)}
            />
            <label htmlFor={`filter-${type}`}>{label}</label>
            <span className="count">{getCount(type)}</span>
          </div>
        ))}
      </div>
    </aside>
  );
}
