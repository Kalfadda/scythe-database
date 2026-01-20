# Scythe Database - Product Requirements Document

## Overview
Scythe Database is a local-only, offline-first desktop application for indexing, browsing, and exporting Unity project assets. It provides fast search, visual previews, dependency tracking, and bundle export capabilities.

## Target Users
- Unity developers managing large projects
- Technical artists organizing asset libraries
- Teams needing to extract asset subsets for sharing

## Core Value Proposition
1. **Speed**: Async scanning with virtualized UI handles 200k+ files without freezing
2. **Dependency Awareness**: Automatically resolves Unity GUID references to show material→texture, prefab→material relationships
3. **Export Bundles**: Extract an asset with all its dependencies, preserving structure
4. **Local-Only**: No cloud, no account, no shared database - your files stay yours

## Functional Requirements

### P0 - Must Have (Phase 1)
| ID | Feature | Description |
|----|---------|-------------|
| F1 | Project Selection | User chooses Unity project root folder; app validates by checking for Assets/ and ProjectSettings/ |
| F2 | Background Scanning | Async filesystem walk with ignore rules (Library/, Temp/, obj/, etc.) |
| F3 | Asset Indexing | Store assets in SQLite with path, type, size, mtime, Unity GUID |
| F4 | Texture Thumbnails | Generate and cache thumbnails for png/jpg/tga images |
| F5 | GUID Dependency Parsing | Parse .mat/.prefab/.asset/.unity YAML for GUID references |
| F6 | Grid View | Virtualized responsive grid showing asset thumbnails and names |
| F7 | Search | Full-text search on asset name and path |
| F8 | Type Filtering | Filter by asset type (model, texture, material, prefab, etc.) |
| F9 | Detail View | Side panel showing metadata, dependencies, and actions |
| F10 | Export File | Copy selected asset to output folder |
| F11 | Export Bundle | Copy asset + resolved dependencies with manifest.json |
| F12 | Settings Persistence | Remember project root and output folder across restarts |
| F13 | Incremental Updates | On focus, process file watcher queue or run quick diff |

### P1 - Should Have (Phase 2)
| ID | Feature | Description |
|----|---------|-------------|
| F14 | Model Stats | Extract vertex/triangle counts from fbx/obj/gltf |
| F15 | 3D Preview | Three.js viewer for model assets |
| F16 | User Metadata | Tags, notes, favorites editable per asset |
| F17 | Saved Views | Store and recall filter/sort configurations |
| F18 | Zip Export | Option to compress bundle exports |
| F19 | Diagnostics Panel | View logs and scan statistics in-app |

### P2 - Nice to Have (Future)
| ID | Feature | Description |
|----|---------|-------------|
| F20 | PSD Support | Thumbnail generation for Photoshop files |
| F21 | Shader Preview | Render shader previews |
| F22 | Batch Operations | Select multiple assets for bulk export |
| F23 | Custom Metadata Fields | User-defined metadata schema |

## Non-Functional Requirements
| Requirement | Target |
|-------------|--------|
| Scan Performance | Index 50k files in < 30 seconds |
| UI Responsiveness | Grid scroll at 60fps with 100k assets |
| Memory | < 500MB RAM during scan |
| Cold Start | App ready in < 2 seconds |
| Database Size | < 100MB for 100k asset index |

## Asset Types Supported
- **Models**: .fbx, .obj, .blend, .dae, .gltf, .glb
- **Textures**: .png, .jpg, .jpeg, .tga, .psd (psd optional)
- **Materials**: .mat
- **Prefabs**: .prefab
- **ScriptableObjects**: .asset
- **Audio**: .wav, .mp3, .ogg
- **Shaders**: .shader, .shadergraph
- **Scenes**: .unity

## Ignore Rules (Default)
```
Library/
Temp/
obj/
Logs/
UserSettings/
.git/
.vs/
Builds/
Build/
```

## Success Criteria
1. App launches and user can select a valid Unity project
2. Scan runs in background without UI freezing
3. Grid populates progressively during scan
4. Search filters assets in < 100ms
5. Detail view opens without perceptible delay
6. Export file copies correctly with relative structure
7. Export bundle gathers dependencies and writes manifest
8. App remembers settings and shows cached index on restart

## Risks & Mitigations
| Risk | Mitigation |
|------|------------|
| Large projects overwhelm memory | Stream files, batch DB writes, limit concurrent operations |
| Unity YAML parsing is complex | Use simple regex/line parsing, mark confidence levels |
| Model thumbnail generation is slow | Defer to Phase 2, show placeholder with stats |
| File watcher floods events | Debounce, coalesce, use bounded queue |
