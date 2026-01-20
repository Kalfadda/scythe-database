# Scythe Database

A lightweight, local-first Unity asset database browser. No cloud. No subscriptions. Just your assets, indexed and searchable.

## The Problem

Unity projects grow. Fast. Before you know it, you're staring at 50,000+ files across hundreds of folders, hunting for that one texture you know exists *somewhere*. The Unity Editor's search is slow. Asset store solutions want your money and your data. You just want to find your files.

## The Solution

Scythe Database indexes your Unity project locally and gives you:

- **Instant search** across all assets with full-text search
- **Visual thumbnails** for textures, materials, and PSD files
- **3D model previews** with orbit controls (OBJ, FBX, GLTF)
- **Dependency tracking** - see what uses what via Unity GUIDs
- **Bundle export** - export an asset with all its dependencies in one click
- **Zero cloud** - everything stays on your machine

## Features

### Asset Indexing
- Scans Unity project structure (Assets, Packages)
- Parses `.meta` files for Unity GUIDs
- Detects asset types: textures, models, materials, prefabs, audio, shaders, scenes
- Full-text search with SQLite FTS5

### Visual Previews
- **Textures**: PNG, JPG, TGA, BMP, GIF, PSD (Photoshop)
- **Materials**: Shows main texture or stylized placeholder
- **3D Models**: Real-time Three.js previews with orbit controls

### Dependency Resolution
- Parses Unity YAML files for GUID references
- Shows "Dependencies" (what this asset uses)
- Shows "Used By" (what uses this asset)
- Recursive dependency tree for bundle export

### Export
- **Export File**: Copy single asset
- **Export Bundle**: Copy asset + all dependencies with manifest

## Tech Stack

- **Frontend**: React + TypeScript + Vite
- **Backend**: Rust + Tauri v2
- **Database**: SQLite with FTS5
- **3D**: Three.js via React Three Fiber
- **Image Processing**: image crate + psd crate

## Getting Started

### Prerequisites
- Node.js 18+
- Rust 1.70+
- [Tauri CLI](https://tauri.app/v1/guides/getting-started/prerequisites)

### Development
```bash
# Install dependencies
npm install

# Run in development mode
npm run tauri dev
```

### Build
```bash
# Build for production
npm run tauri build
```

## Usage

1. **Select Project** - Choose your Unity project root folder
2. **Wait for Scan** - Initial indexing takes a moment (progress shown)
3. **Browse & Search** - Filter by type, search by name
4. **Select Asset** - View details, preview, dependencies
5. **Export** - Set output folder, export single file or bundle

## License

MIT

---

Built with Rust and stubbornness.
