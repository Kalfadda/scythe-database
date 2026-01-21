export type AssetType =
  | 'texture'
  | 'model'
  | 'material'
  | 'prefab'
  | 'audio'
  | 'shader'
  | 'scene'
  | 'scriptable_object'
  | 'unknown';

export interface Asset {
  id: string;
  project_id: string;
  absolute_path: string;
  relative_path: string;
  file_name: string;
  extension: string;
  asset_type: AssetType;
  size_bytes: number;
  modified_time: number;
  content_hash: string | null;
  unity_guid: string | null;
  import_type: string | null;
  thumbnail_path: string | null;
}

export interface Dependency {
  id: string;
  from_asset_id: string;
  to_asset_id: string | null;
  to_guid: string;
  relation_type: string;
  confidence: 'high' | 'medium' | 'low';
}

export interface Project {
  id: string;
  root_path: string;
  name: string;
  last_scan_time: number | null;
  file_count: number;
}

export interface ScanProgress {
  scanned: number;
  total: number | null;
  current_path: string;
  phase: 'counting' | 'walking' | 'indexing' | 'dependencies' | 'thumbnails' | 'complete' | 'cancelled';
  /** Number of unchanged files skipped during re-scan */
  skipped: number | null;
  /** Number of new or changed files processed */
  changed: number | null;
}

export interface ThumbnailProgress {
  generated: number;
  total: number;
  phase: 'counting' | 'generating' | 'generating_models' | 'complete' | 'cancelled';
}

export interface ModelAssetInfo {
  id: string;
  absolute_path: string;
  extension: string;
  modified_time: number;
}

export interface AssetFilter {
  search_query: string;
  asset_types: AssetType[];
  page: number;
  page_size: number;
}

export interface AppSettings {
  project_root: string | null;
  output_folder: string | null;
  ignore_patterns: string[];
  thumbnail_size: number;
  scan_on_focus: boolean;
}

export interface ExportResult {
  success: boolean;
  exported_files: string[];
  manifest_path: string | null;
  error: string | null;
}

export interface TypeCount {
  asset_type: AssetType;
  count: number;
}

export interface MaterialTexture {
  slot_name: string;
  texture_guid: string | null;
  texture_path: string | null;
}

export interface MaterialInfo {
  shader_name: string | null;
  textures: MaterialTexture[];
}

export interface ModelInfo {
  vertex_count: number | null;
  triangle_count: number | null;
  submesh_count: number | null;
  has_normals: boolean;
  has_uvs: boolean;
  bounds: [number, number, number, number, number, number] | null;
}

export interface BundleAssetInfo {
  id: string;
  file_name: string;
  relative_path: string;
  asset_type: string;
  size_bytes: number;
}

export interface BundlePreview {
  root_asset: BundleAssetInfo;
  dependencies: BundleAssetInfo[];
  total_size_bytes: number;
}
