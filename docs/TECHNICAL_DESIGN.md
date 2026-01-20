# Scythe Database - Technical Design

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                        React Frontend                            │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────────────┐   │
│  │  Grid    │ │  Detail  │ │  Search  │ │  Export Dialog   │   │
│  │  View    │ │  Panel   │ │  Bar     │ │                  │   │
│  └────┬─────┘ └────┬─────┘ └────┬─────┘ └────────┬─────────┘   │
│       │            │            │                 │              │
│  ┌────┴────────────┴────────────┴─────────────────┴────────┐    │
│  │                   State Management (Zustand)             │    │
│  └────────────────────────────┬─────────────────────────────┘    │
└───────────────────────────────┼──────────────────────────────────┘
                                │ Tauri IPC (invoke)
┌───────────────────────────────┼──────────────────────────────────┐
│                        Rust Backend                               │
│  ┌────────────────────────────┴─────────────────────────────┐    │
│  │                    Command Handlers                       │    │
│  └────┬─────────┬─────────┬─────────┬─────────┬────────────┘    │
│       │         │         │         │         │                  │
│  ┌────┴───┐ ┌───┴───┐ ┌───┴───┐ ┌───┴───┐ ┌───┴───┐            │
│  │Scanner │ │Indexer│ │  Deps │ │Preview│ │Export │            │
│  │Service │ │Service│ │Resolver│ │ Cache │ │Service│            │
│  └────┬───┘ └───┬───┘ └───┬───┘ └───┬───┘ └───────┘            │
│       │         │         │         │                            │
│  ┌────┴─────────┴─────────┴─────────┴────────────────────────┐  │
│  │                    SQLite Database                         │  │
│  └────────────────────────────────────────────────────────────┘  │
│                                                                   │
│  ┌──────────────────┐  ┌──────────────────┐                      │
│  │  File Watcher    │  │  Thumbnail FS    │                      │
│  │  (notify crate)  │  │  Cache           │                      │
│  └──────────────────┘  └──────────────────┘                      │
└───────────────────────────────────────────────────────────────────┘
```

## Component Details

### 1. Scanner Service (`src-tauri/src/scanner/`)
**Responsibility**: Walk filesystem, classify files, read Unity .meta files

**Key Functions**:
- `scan_project(root: PathBuf, ignore_patterns: Vec<String>) -> ScanResult`
- `classify_file(path: &Path) -> AssetType`
- `parse_meta_file(path: &Path) -> Option<MetaInfo>`

**Implementation**:
- Uses `walkdir` crate for recursive traversal
- Spawns on Tokio blocking thread pool
- Yields batches of 100 files at a time
- Respects cancellation via `CancellationToken`
- Emits progress events via Tauri events

### 2. Indexer Service (`src-tauri/src/indexer/`)
**Responsibility**: Write assets to SQLite, de-duplicate, manage transactions

**Key Functions**:
- `upsert_assets(assets: Vec<Asset>) -> Result<usize>`
- `delete_missing(valid_paths: HashSet<PathBuf>) -> Result<usize>`
- `get_assets(filter: AssetFilter, page: Page) -> Vec<Asset>`
- `search_assets(query: &str, limit: usize) -> Vec<Asset>`

**Implementation**:
- Uses `rusqlite` with connection pooling via `r2d2`
- Batch inserts in transactions of 500 rows
- Full-text search via SQLite FTS5
- Indexes on: relative_path, asset_type, unity_guid

### 3. Dependency Resolver (`src-tauri/src/deps/`)
**Responsibility**: Parse Unity YAML assets for GUID references

**Key Functions**:
- `extract_guids(content: &str) -> Vec<GuidReference>`
- `resolve_dependencies(asset_id: Uuid) -> Vec<Dependency>`
- `get_dependents(asset_id: Uuid) -> Vec<Dependency>`

**Implementation**:
- Simple line-by-line YAML parsing (not full YAML parser)
- Regex pattern: `guid: ([a-f0-9]{32})`
- Maps GUIDs to assets via indexed lookup
- Stores confidence level based on context

### 4. Preview Cache (`src-tauri/src/previews/`)
**Responsibility**: Generate and cache thumbnails

**Key Functions**:
- `get_or_create_thumbnail(asset: &Asset) -> Result<PathBuf>`
- `invalidate_thumbnail(asset_id: Uuid)`
- `cleanup_orphaned_thumbnails()`

**Implementation**:
- Thumbnails stored in app data dir: `{app_data}/thumbnails/{hash}.jpg`
- Uses `image` crate for resizing (128x128 default)
- TGA support via `image` crate's tga feature
- Content-addressed: filename is first 16 bytes of file hash
- Background generation queue with priority

### 5. Export Service (`src-tauri/src/export/`)
**Responsibility**: Copy assets and dependencies to destination

**Key Functions**:
- `export_file(asset_id: Uuid, dest: PathBuf) -> Result<ExportResult>`
- `export_bundle(asset_id: Uuid, dest: PathBuf, options: BundleOptions) -> Result<ExportResult>`

**Implementation**:
- Preserves relative path structure from project root
- Recursively gathers dependencies up to configured depth
- Writes manifest.json with asset list and dependency graph
- Optional zip via `zip` crate

### 6. Settings Service (`src-tauri/src/settings/`)
**Responsibility**: Persist and load app configuration

**Storage**: JSON file at `{app_data}/settings.json`

```json
{
  "project_root": "C:/Projects/MyUnityGame",
  "output_folder": "C:/Exports",
  "ignore_patterns": ["Library/", "Temp/", ...],
  "thumbnail_size": 128,
  "scan_on_focus": true
}
```

## Database Schema

```sql
-- Projects table (support multiple projects)
CREATE TABLE projects (
    id TEXT PRIMARY KEY,           -- UUID
    root_path TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    last_scan_time INTEGER,        -- Unix timestamp
    file_count INTEGER DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

-- Main assets table
CREATE TABLE assets (
    id TEXT PRIMARY KEY,           -- UUID
    project_id TEXT NOT NULL,
    absolute_path TEXT NOT NULL,
    relative_path TEXT NOT NULL,
    file_name TEXT NOT NULL,
    extension TEXT NOT NULL,
    asset_type TEXT NOT NULL,      -- 'model', 'texture', 'material', etc.
    size_bytes INTEGER NOT NULL,
    modified_time INTEGER NOT NULL,
    content_hash TEXT,             -- Optional, computed on demand
    unity_guid TEXT,               -- From .meta file
    import_type TEXT,              -- Unity importer type guess
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
    UNIQUE(project_id, relative_path)
);

CREATE INDEX idx_assets_project ON assets(project_id);
CREATE INDEX idx_assets_type ON assets(asset_type);
CREATE INDEX idx_assets_guid ON assets(unity_guid);
CREATE INDEX idx_assets_relative_path ON assets(relative_path);

-- Full-text search virtual table
CREATE VIRTUAL TABLE assets_fts USING fts5(
    file_name,
    relative_path,
    content=assets,
    content_rowid=rowid
);

-- Triggers to keep FTS in sync
CREATE TRIGGER assets_ai AFTER INSERT ON assets BEGIN
    INSERT INTO assets_fts(rowid, file_name, relative_path)
    VALUES (NEW.rowid, NEW.file_name, NEW.relative_path);
END;

CREATE TRIGGER assets_ad AFTER DELETE ON assets BEGIN
    INSERT INTO assets_fts(assets_fts, rowid, file_name, relative_path)
    VALUES ('delete', OLD.rowid, OLD.file_name, OLD.relative_path);
END;

CREATE TRIGGER assets_au AFTER UPDATE ON assets BEGIN
    INSERT INTO assets_fts(assets_fts, rowid, file_name, relative_path)
    VALUES ('delete', OLD.rowid, OLD.file_name, OLD.relative_path);
    INSERT INTO assets_fts(rowid, file_name, relative_path)
    VALUES (NEW.rowid, NEW.file_name, NEW.relative_path);
END;

-- Dependencies table
CREATE TABLE dependencies (
    id TEXT PRIMARY KEY,
    from_asset_id TEXT NOT NULL,
    to_asset_id TEXT,              -- NULL if unresolved
    to_guid TEXT,                  -- Original GUID reference
    relation_type TEXT NOT NULL,   -- 'material_texture', 'prefab_material', etc.
    confidence TEXT NOT NULL,      -- 'high', 'medium', 'low'
    created_at INTEGER NOT NULL,
    FOREIGN KEY (from_asset_id) REFERENCES assets(id) ON DELETE CASCADE,
    FOREIGN KEY (to_asset_id) REFERENCES assets(id) ON DELETE CASCADE
);

CREATE INDEX idx_deps_from ON dependencies(from_asset_id);
CREATE INDEX idx_deps_to ON dependencies(to_asset_id);
CREATE INDEX idx_deps_guid ON dependencies(to_guid);

-- User metadata (Phase 2, but schema now)
CREATE TABLE asset_metadata (
    asset_id TEXT PRIMARY KEY,
    tags TEXT,                     -- JSON array
    notes TEXT,
    is_favorite INTEGER DEFAULT 0,
    rating INTEGER,
    custom_fields TEXT,            -- JSON object
    updated_at INTEGER NOT NULL,
    FOREIGN KEY (asset_id) REFERENCES assets(id) ON DELETE CASCADE
);

-- Preview cache tracking
CREATE TABLE preview_cache (
    asset_id TEXT PRIMARY KEY,
    thumb_path TEXT NOT NULL,
    thumb_size INTEGER NOT NULL,
    version_key TEXT NOT NULL,     -- content_hash or mtime+size
    created_at INTEGER NOT NULL,
    FOREIGN KEY (asset_id) REFERENCES assets(id) ON DELETE CASCADE
);
```

## Data Flow

### Scan Flow
```
1. User clicks "Scan" or selects project
2. Frontend invokes `scan_project` command
3. Backend spawns scanner task on blocking pool
4. Scanner walks filesystem, yields batches
5. Each batch: indexer upserts to DB
6. Backend emits progress events to frontend
7. Frontend updates grid progressively
8. On complete: update project.last_scan_time
```

### Search Flow
```
1. User types in search box (debounced 200ms)
2. Frontend invokes `search_assets` command
3. Backend queries FTS5 with prefix match
4. Returns paginated results
5. Frontend updates grid
```

### Export Bundle Flow
```
1. User clicks "Export Bundle" on asset
2. Frontend invokes `export_bundle` command
3. Backend resolves dependencies recursively
4. For each dependency: resolve GUID -> path
5. Copy all files preserving structure
6. Generate manifest.json
7. Return export summary
```

## Frontend State (Zustand)

```typescript
interface AppState {
  // Project
  projectRoot: string | null;
  projectId: string | null;

  // Assets
  assets: Asset[];
  totalCount: number;
  isLoading: boolean;
  scanProgress: ScanProgress | null;

  // Filters
  searchQuery: string;
  selectedTypes: AssetType[];
  sortBy: SortField;
  sortOrder: 'asc' | 'desc';

  // Selection
  selectedAssetId: string | null;

  // Settings
  outputFolder: string | null;

  // Actions
  setProjectRoot: (path: string) => void;
  loadAssets: (page: number) => Promise<void>;
  search: (query: string) => void;
  selectAsset: (id: string) => void;
  exportFile: (id: string) => Promise<void>;
  exportBundle: (id: string) => Promise<void>;
}
```

## File Structure

```
scythe-database/
├── docs/
│   ├── PRD.md
│   └── TECHNICAL_DESIGN.md
├── src/                          # React frontend
│   ├── components/
│   │   ├── AssetGrid.tsx
│   │   ├── AssetTile.tsx
│   │   ├── DetailPanel.tsx
│   │   ├── SearchBar.tsx
│   │   ├── FilterSidebar.tsx
│   │   ├── ProjectSelector.tsx
│   │   └── ExportDialog.tsx
│   ├── hooks/
│   │   ├── useAssets.ts
│   │   ├── useScan.ts
│   │   └── useExport.ts
│   ├── state/
│   │   └── store.ts
│   ├── types/
│   │   └── index.ts
│   ├── lib/
│   │   └── tauri.ts
│   ├── App.tsx
│   ├── main.tsx
│   └── index.css
├── src-tauri/
│   ├── src/
│   │   ├── scanner/
│   │   │   ├── mod.rs
│   │   │   └── classifier.rs
│   │   ├── indexer/
│   │   │   ├── mod.rs
│   │   │   └── db.rs
│   │   ├── deps/
│   │   │   ├── mod.rs
│   │   │   └── parser.rs
│   │   ├── previews/
│   │   │   ├── mod.rs
│   │   │   └── thumbnail.rs
│   │   ├── export/
│   │   │   └── mod.rs
│   │   ├── settings/
│   │   │   └── mod.rs
│   │   ├── commands.rs
│   │   ├── state.rs
│   │   ├── error.rs
│   │   └── lib.rs
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   └── build.rs
├── package.json
├── tsconfig.json
├── vite.config.ts
└── README.md
```

## Risks & Tradeoffs

| Decision | Tradeoff | Rationale |
|----------|----------|-----------|
| rusqlite over sqlx | No async, but simpler | SQLite is fast enough sync; avoid async complexity |
| Line-based YAML parsing | May miss nested refs | Full YAML parsing is slow; regex catches 90% |
| Thumbnails at 128px | Storage vs quality | Good balance for grid; detail view shows full |
| No model thumbnails Phase 1 | Missing feature | Complexity/dependencies too high for MVP |
| FTS5 for search | Limited ranking | Good enough; can add better ranking later |
