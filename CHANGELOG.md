# Changelog

All notable changes to Scythe Database will be documented in this file.

## [Unreleased]

### Added
- **Cancellable operations**: All long-running operations (scan, dependency resolution, thumbnail generation) can now be cancelled via the Cancel button
- **Accurate progress tracking**: File counting phase before scanning provides accurate progress percentages
- **Regenerate button**: Replaced "Refresh" with "Regenerate" that performs a full re-scan and thumbnail regeneration
- **Separate thumbnail generation phase**: Thumbnails now generated in a dedicated phase with their own progress bar
- **Client-side 3D model thumbnails**: Model thumbnails (OBJ, FBX, GLTF) are now generated in the browser using Three.js
- **Dependency resolution progress**: Shows progress during the dependency analysis phase
- **TOO_LARGE/UNSUPPORTED markers**: Files that cannot have thumbnails are properly marked instead of repeatedly retried

### Changed
- Scan phases are now clearly separated: counting → indexing → dependencies → complete
- Thumbnail generation runs after scan completion rather than blocking it
- Progress bars now show accurate percentages based on pre-counted totals

### Fixed
- Progress bar accuracy during long scans
- UI responsiveness during large folder scans

## [0.2.0] - 2025-01-20

### Added
- Persistent IndexedDB thumbnail caching with preloading
- CLAUDE.md for Claude Code guidance

### Fixed
- Pagination when window is maximized
- Scroll and pagination issues

## [0.1.0] - 2025-01-19

### Added
- Initial release
- 3D model thumbnails with orbit controls
- Multi-folder project support
- Asset indexing with full-text search
- Visual previews for textures, materials, and 3D models
- Unity GUID-based dependency tracking
- Bundle export with dependencies
- SQLite database with FTS5 search
