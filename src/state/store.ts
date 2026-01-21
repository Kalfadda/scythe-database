import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type { Asset, AssetType, ScanProgress, ThumbnailProgress, ModelAssetInfo, Project, TypeCount, Dependency } from '../types';
import { generateAllModelThumbnails } from '../services/modelThumbnailCache';

interface AppState {
  // Project
  project: Project | null;
  projectRoot: string | null;
  outputFolder: string | null;

  // Assets
  assets: Asset[];
  totalCount: number;
  isLoading: boolean;
  scanProgress: ScanProgress | null;
  thumbnailProgress: ThumbnailProgress | null;
  typeCounts: TypeCount[];

  // Regeneration state
  isRegenerating: boolean;

  // Filters
  searchQuery: string;
  selectedTypes: AssetType[];
  page: number;
  pageSize: number;

  // Selection
  selectedAssetId: string | null;
  selectedAsset: Asset | null;
  dependencies: Dependency[];
  dependents: Dependency[];

  // Actions
  setProjectRoot: (path: string) => Promise<void>;
  setOutputFolder: (path: string) => Promise<void>;
  loadAssets: () => Promise<void>;
  loadMoreAssets: () => Promise<void>;
  search: (query: string) => void;
  toggleTypeFilter: (type: AssetType) => void;
  selectAsset: (id: string | null) => Promise<void>;
  startScan: () => Promise<void>;
  updateScanProgress: (progress: ScanProgress) => void;
  regenerateDatabase: () => Promise<void>;
  cancelRegeneration: () => Promise<void>;
  regenerateThumbnails: () => Promise<void>;
  updateThumbnailProgress: (progress: ThumbnailProgress) => void;
  exportFile: (id: string) => Promise<void>;
  exportBundle: (id: string) => Promise<void>;
  loadSettings: () => Promise<void>;
  refreshAssets: () => Promise<void>;
  loadTypeCounts: () => Promise<void>;
}

export const useStore = create<AppState>((set, get) => ({
  project: null,
  projectRoot: null,
  outputFolder: null,
  assets: [],
  totalCount: 0,
  isLoading: false,
  scanProgress: null,
  thumbnailProgress: null,
  typeCounts: [],
  isRegenerating: false,
  searchQuery: '',
  selectedTypes: [],
  page: 0,
  pageSize: 50,
  selectedAssetId: null,
  selectedAsset: null,
  dependencies: [],
  dependents: [],

  setProjectRoot: async (path: string) => {
    try {
      const project = await invoke<Project>('set_project_root', { path });
      set({ project, projectRoot: path, assets: [], totalCount: 0, page: 0 });
      await get().startScan();
    } catch (error) {
      console.error('Failed to set project root:', error);
    }
  },

  setOutputFolder: async (path: string) => {
    try {
      await invoke('set_output_folder', { path });
      set({ outputFolder: path });
    } catch (error) {
      console.error('Failed to set output folder:', error);
    }
  },

  loadAssets: async () => {
    const { project, searchQuery, selectedTypes, page, pageSize } = get();
    if (!project) return;

    set({ isLoading: true });
    try {
      const result = await invoke<{ assets: Asset[]; total: number }>('get_assets', {
        projectId: project.id,
        searchQuery: searchQuery || null,
        assetTypes: selectedTypes.length > 0 ? selectedTypes : null,
        page,
        pageSize,
      });

      set({
        assets: page === 0 ? result.assets : [...get().assets, ...result.assets],
        totalCount: result.total,
        isLoading: false
      });
    } catch (error) {
      console.error('Failed to load assets:', error);
      set({ isLoading: false });
    }
  },

  loadMoreAssets: async () => {
    const { page, totalCount, assets, isLoading } = get();
    // Don't load more if already loading or if we have all assets
    if (isLoading || assets.length >= totalCount) return;

    set({ page: page + 1 });
    await get().loadAssets();
  },

  search: (query: string) => {
    // Don't clear assets immediately - show loading state instead
    set({ searchQuery: query, page: 0, isLoading: true });
    get().loadAssets();
  },

  toggleTypeFilter: (type: AssetType) => {
    const { selectedTypes } = get();
    const newTypes = selectedTypes.includes(type)
      ? selectedTypes.filter(t => t !== type)
      : [...selectedTypes, type];
    // Don't clear assets - keep showing old ones while loading new
    set({ selectedTypes: newTypes, page: 0, isLoading: true });
    get().loadAssets();
  },

  selectAsset: async (id: string | null) => {
    set({ selectedAssetId: id });

    if (!id) {
      set({ selectedAsset: null, dependencies: [], dependents: [] });
      return;
    }

    try {
      const [asset, dependencies, dependents] = await Promise.all([
        invoke<Asset>('get_asset', { id }),
        invoke<Dependency[]>('get_dependencies', { assetId: id }),
        invoke<Dependency[]>('get_dependents', { assetId: id }),
      ]);
      set({ selectedAsset: asset, dependencies, dependents });
    } catch (error) {
      console.error('Failed to load asset details:', error);
    }
  },

  startScan: async () => {
    const { project } = get();
    if (!project) return;

    set({ scanProgress: { scanned: 0, total: null, current_path: '', phase: 'walking' } });
    try {
      await invoke('start_scan', { projectId: project.id });
    } catch (error) {
      console.error('Failed to start scan:', error);
      set({ scanProgress: null });
    }
  },

  updateScanProgress: (progress: ScanProgress) => {
    set({ scanProgress: progress });

    if (progress.phase === 'complete') {
      set({ scanProgress: null });
      get().loadAssets();
      get().loadTypeCounts();

      // If this was a full regeneration, continue with thumbnail generation
      const { isRegenerating } = get();
      if (isRegenerating) {
        get().regenerateThumbnails();
      }
    } else if (progress.phase === 'cancelled') {
      set({ scanProgress: null, isRegenerating: false });
      get().loadAssets();
      get().loadTypeCounts();
    }
  },

  regenerateDatabase: async () => {
    const { project } = get();
    if (!project) return;

    // Set flag to trigger thumbnail generation after scan completes
    set({ isRegenerating: true });
    set({ scanProgress: { scanned: 0, total: null, current_path: '', phase: 'counting' } });
    try {
      await invoke('start_scan', { projectId: project.id });
    } catch (error) {
      console.error('Failed to start scan:', error);
      set({ scanProgress: null, isRegenerating: false });
    }
  },

  cancelRegeneration: async () => {
    try {
      await invoke('cancel_operation');
    } catch (error) {
      console.error('Failed to cancel operation:', error);
    }
  },

  regenerateThumbnails: async () => {
    const { project } = get();
    if (!project) return;

    set({ thumbnailProgress: { generated: 0, total: 0, phase: 'counting' } });
    try {
      await invoke('regenerate_thumbnails', { projectId: project.id });
    } catch (error) {
      console.error('Failed to regenerate thumbnails:', error);
      set({ thumbnailProgress: null, isRegenerating: false });
    }
  },

  updateThumbnailProgress: async (progress: ThumbnailProgress) => {
    set({ thumbnailProgress: progress });

    // Handle cancellation
    if (progress.phase === 'cancelled') {
      set({ thumbnailProgress: null, isRegenerating: false });
      return;
    }

    // When backend texture generation completes, start model thumbnail generation
    if (progress.phase === 'complete') {
      const { project } = get();
      if (!project) {
        set({ thumbnailProgress: null, isRegenerating: false });
        return;
      }

      try {
        // Fetch model assets
        set({ thumbnailProgress: { generated: 0, total: 0, phase: 'generating_models' } });
        const modelAssets = await invoke<ModelAssetInfo[]>('get_model_assets_for_thumbnails', { projectId: project.id });

        if (modelAssets.length === 0) {
          set({ thumbnailProgress: null, isRegenerating: false });
          return;
        }

        // Generate model thumbnails client-side
        await generateAllModelThumbnails(modelAssets, (generated, total) => {
          set({ thumbnailProgress: { generated, total, phase: 'generating_models' } });
        });

        set({ thumbnailProgress: null, isRegenerating: false });
      } catch (error) {
        console.error('Failed to generate model thumbnails:', error);
        set({ thumbnailProgress: null, isRegenerating: false });
      }
    }
  },

  exportFile: async (id: string) => {
    const { outputFolder } = get();
    if (!outputFolder) {
      console.error('No output folder set');
      return;
    }

    try {
      await invoke('export_file', { assetId: id, destFolder: outputFolder });
    } catch (error) {
      console.error('Failed to export file:', error);
    }
  },

  exportBundle: async (id: string) => {
    const { outputFolder } = get();
    if (!outputFolder) {
      console.error('No output folder set');
      return;
    }

    try {
      await invoke('export_bundle', { assetId: id, destFolder: outputFolder });
    } catch (error) {
      console.error('Failed to export bundle:', error);
    }
  },

  loadSettings: async () => {
    try {
      const settings = await invoke<{ project_root: string | null; output_folder: string | null }>('get_settings');
      set({
        projectRoot: settings.project_root,
        outputFolder: settings.output_folder,
      });

      if (settings.project_root) {
        const project = await invoke<Project | null>('get_current_project');
        if (project) {
          set({ project });
          await get().loadAssets();
          await get().loadTypeCounts();
        }
      }
    } catch (error) {
      console.error('Failed to load settings:', error);
    }
  },

  refreshAssets: async () => {
    const { project, searchQuery, selectedTypes, pageSize } = get();
    if (!project) return;

    // Don't clear assets - just fetch fresh data
    try {
      const result = await invoke<{ assets: Asset[]; total: number }>('get_assets', {
        projectId: project.id,
        searchQuery: searchQuery || null,
        assetTypes: selectedTypes.length > 0 ? selectedTypes : null,
        page: 0,
        pageSize,
      });

      set({
        assets: result.assets,
        totalCount: result.total,
        page: 0,
      });
    } catch (error) {
      console.error('Failed to refresh assets:', error);
    }
  },

  loadTypeCounts: async () => {
    const { project } = get();
    if (!project) return;

    try {
      const counts = await invoke<TypeCount[]>('get_type_counts', { projectId: project.id });
      set({ typeCounts: counts });
    } catch (error) {
      console.error('Failed to load type counts:', error);
    }
  },
}));
