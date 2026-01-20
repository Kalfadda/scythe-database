# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Development Commands

```bash
# Development
npm install              # Install dependencies
npm run tauri dev        # Run app in development mode (Vite + Tauri)

# Production
npm run build            # Build frontend (TypeScript + Vite)
npm run tauri build      # Build production desktop app

# Rust only
cd src-tauri && cargo check    # Type-check Rust code
cd src-tauri && cargo build    # Build Rust backend
```

## Architecture Overview

Scythe Database is a Tauri v2 desktop app with a React frontend and Rust backend.

### Frontend (React + TypeScript + Vite)

- **State**: Zustand store in `src/state/store.ts` - all UI state and actions
- **Components**: `src/components/` - React components
- **Types**: `src/types/index.ts` - shared TypeScript interfaces
- **Services**: `src/services/` - thumbnail caching utilities

**Key pattern**: Components call store actions → store invokes Tauri commands → updates state

```typescript
// Store actions use Tauri invoke for backend calls
const result = await invoke<ResponseType>('command_name', { args });
```

### Backend (Rust + Tauri)

- **commands.rs**: Tauri command handlers (IPC endpoints) - `#[tauri::command]` functions
- **state.rs**: `AppState` struct with `Arc<Database>` and `Arc<RwLock<Settings>>`
- **db.rs**: SQLite models and queries (rusqlite + r2d2 pooling)
- **scanner.rs**: Filesystem walking and file classification
- **indexer.rs**: Batch asset upserts to database
- **deps.rs**: Unity GUID dependency resolution from YAML files
- **previews.rs**: Thumbnail generation (PNG/JPG/TGA/PSD support)
- **export.rs**: Asset + dependency export with manifest

**Key pattern**: Commands are async, heavy work uses `tokio::task::spawn_blocking()`

### Database (SQLite + FTS5)

Tables: `projects`, `assets`, `dependencies`, `preview_cache`

Key indexes: `(project_id)`, `(project_id, asset_type)`, `(unity_guid)`, `(relative_path)`

FTS5 virtual table `assets_fts` with triggers for full-text search.

### Frontend-Backend Communication

- **Commands**: `invoke()` calls to Rust functions
- **Events**: Real-time updates via `scan-progress` and `assets-updated` events

## Key Conventions

- Batch size of 25 files during scanning for responsive UI
- Page size of 50 assets for grid pagination
- Thumbnails are 128x128, content-addressed by file hash
- PSD parsing wrapped in `catch_unwind` (crate can panic)
- Multi-phase scan: indexing → dependencies → thumbnails (thumbnails continue in background after "complete")

## Adding New Tauri Commands

1. Add function in `src-tauri/src/commands.rs` with `#[tauri::command]`
2. Register in `src-tauri/src/lib.rs` in `generate_handler![]`
3. Call from frontend with `invoke<ReturnType>('command_name', { args })`
