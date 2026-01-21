use crate::db::Asset;
use crate::error::{AppError, AppResult};
use jwalk::WalkDir;
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Map of relative_path -> (id, modified_time, size_bytes) for existing assets
pub type ExistingAssetMap = HashMap<String, (String, i64, i64)>;

/// Statistics about a scan operation
#[derive(Debug, Clone, Copy, Default)]
pub struct ScanStats {
    pub total_files: usize,
    pub unchanged_skipped: usize,
    pub new_or_changed: usize,
}

pub struct Scanner {
    ignore_patterns: Vec<String>,
}

impl Scanner {
    pub fn new(ignore_patterns: Vec<String>) -> Self {
        Self { ignore_patterns }
    }

    #[allow(dead_code)]
    pub fn is_valid_unity_project(root: &Path) -> bool {
        root.join("Assets").is_dir() && root.join("ProjectSettings").is_dir()
    }

    pub fn is_valid_folder(root: &Path) -> bool {
        root.is_dir()
    }

    #[allow(dead_code)]
    pub fn scan(&self, root: &Path, project_id: &str) -> AppResult<Vec<Asset>> {
        if !Self::is_valid_folder(root) {
            return Err(AppError::InvalidProject(
                "Not a valid folder".to_string(),
            ));
        }

        let mut assets = Vec::new();
        let now = chrono::Utc::now().timestamp();
        let root_path = root.to_path_buf();
        let ignore_patterns = self.ignore_patterns.clone();

        for entry in WalkDir::new(root)
            .follow_links(false)
            .process_read_dir(move |_depth, _path, _state, children| {
                // Filter out ignored directories to prevent descending into them
                children.retain(|entry| {
                    entry.as_ref().map_or(true, |e| {
                        !should_ignore_path(&e.path(), &root_path, &ignore_patterns)
                    })
                });
            })
        {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };

            if !entry.file_type().is_file() {
                continue;
            }

            let path = entry.path();

            // Skip .meta files for main asset list (we process them separately)
            if path.extension().map(|e| e == "meta").unwrap_or(false) {
                continue;
            }

            let asset_type = classify_file(&path);
            if asset_type == "unknown" {
                continue;
            }

            let relative_path = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .to_string();

            let file_name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();

            let extension = path
                .extension()
                .map(|e| e.to_string_lossy().to_string())
                .unwrap_or_default();

            let metadata = fs::metadata(&path)?;
            let size_bytes = metadata.len() as i64;

            let modified_time = metadata
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);

            // Try to read Unity GUID from .meta file
            let unity_guid = read_unity_guid(&path.with_extension(format!(
                "{}.meta",
                extension
            )));

            let asset = Asset {
                id: uuid::Uuid::new_v4().to_string(),
                project_id: project_id.to_string(),
                absolute_path: path.to_string_lossy().to_string(),
                relative_path,
                file_name,
                extension,
                asset_type: asset_type.to_string(),
                size_bytes,
                modified_time,
                content_hash: None,
                unity_guid,
                import_type: None,
                thumbnail_path: None,
                created_at: now,
                updated_at: now,
            };

            assets.push(asset);
        }

        Ok(assets)
    }

    fn should_ignore(&self, path: &Path, root: &Path) -> bool {
        should_ignore_path(path, root, &self.ignore_patterns)
    }
}

/// Standalone helper for ignore checking (usable in closures)
fn should_ignore_path(path: &Path, root: &Path, ignore_patterns: &[String]) -> bool {
    let relative = path.strip_prefix(root).unwrap_or(path);
    let relative_str = relative.to_string_lossy();

    for pattern in ignore_patterns {
        let pattern_normalized = pattern.trim_end_matches('/');
        if relative_str.starts_with(pattern_normalized)
            || relative_str.contains(&format!("/{}", pattern_normalized))
            || relative_str.contains(&format!("\\{}", pattern_normalized))
        {
            return true;
        }
    }

    false
}

pub fn classify_file(path: &Path) -> &'static str {
    let extension = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    match extension.as_str() {
        // Textures
        "png" | "jpg" | "jpeg" | "tga" | "psd" | "bmp" | "gif" | "exr" | "hdr" => "texture",

        // Models
        "fbx" | "obj" | "blend" | "dae" | "gltf" | "glb" | "3ds" | "max" => "model",

        // Materials
        "mat" => "material",

        // Prefabs
        "prefab" => "prefab",

        // Audio
        "wav" | "mp3" | "ogg" | "aiff" | "aif" | "flac" => "audio",

        // Shaders
        "shader" | "shadergraph" | "shadersubgraph" | "compute" | "cginc" | "hlsl" | "glsl" => {
            "shader"
        }

        // Scenes
        "unity" => "scene",

        // ScriptableObjects and other assets
        "asset" => "scriptable_object",

        // Animation
        "anim" | "controller" | "overrideController" => "unknown", // Could add animation type later

        // Scripts (exclude from main index for now)
        "cs" | "js" | "boo" => "unknown",

        _ => "unknown",
    }
}

fn read_unity_guid(meta_path: &Path) -> Option<String> {
    if !meta_path.exists() {
        return None;
    }

    let content = fs::read_to_string(meta_path).ok()?;

    // Parse GUID from Unity .meta file
    // Format: guid: 32hexcharacters
    let re = Regex::new(r"guid:\s*([a-f0-9]{32})").ok()?;

    re.captures(&content)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_string())
}

/// Count files that would be scanned (quick pre-count for progress)
pub fn count_scannable_files(
    root: &Path,
    ignore_patterns: &[String],
    cancel_flag: Arc<AtomicBool>,
    mut progress_callback: impl FnMut(usize),
) -> AppResult<usize> {
    if !Scanner::is_valid_folder(root) {
        return Err(AppError::InvalidProject(
            "Not a valid folder".to_string(),
        ));
    }

    let mut count = 0;
    let root_path = root.to_path_buf();
    let patterns = ignore_patterns.to_vec();

    for entry in WalkDir::new(root)
        .follow_links(false)
        .process_read_dir(move |_depth, _path, _state, children| {
            children.retain(|entry| {
                entry.as_ref().map_or(true, |e| {
                    !should_ignore_path(&e.path(), &root_path, &patterns)
                })
            });
        })
    {
        // Check cancellation
        if cancel_flag.load(Ordering::SeqCst) {
            return Ok(count);
        }

        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path();

        if path.extension().map(|e| e == "meta").unwrap_or(false) {
            continue;
        }

        let asset_type = classify_file(&path);
        if asset_type == "unknown" {
            continue;
        }

        count += 1;

        // Report progress every 100 files
        if count % 100 == 0 {
            progress_callback(count);
        }
    }

    progress_callback(count);
    Ok(count)
}

pub fn scan_files_batch(
    root: &Path,
    project_id: &str,
    ignore_patterns: &[String],
    batch_size: usize,
    cancel_flag: Arc<AtomicBool>,
    existing_assets: Option<&ExistingAssetMap>,
    mut callback: impl FnMut(Vec<Asset>, usize, &str) -> bool,  // Returns false to stop
) -> AppResult<(usize, ScanStats)> {
    if !Scanner::is_valid_folder(root) {
        return Err(AppError::InvalidProject(
            "Not a valid folder".to_string(),
        ));
    }

    let mut batch = Vec::with_capacity(batch_size);
    let mut total_count = 0;
    let mut stats = ScanStats::default();
    let now = chrono::Utc::now().timestamp();
    let root_path = root.to_path_buf();
    let patterns = ignore_patterns.to_vec();

    for entry in WalkDir::new(root)
        .follow_links(false)
        .process_read_dir(move |_depth, _path, _state, children| {
            children.retain(|entry| {
                entry.as_ref().map_or(true, |e| {
                    !should_ignore_path(&e.path(), &root_path, &patterns)
                })
            });
        })
    {
        // Check cancellation
        if cancel_flag.load(Ordering::SeqCst) {
            return Ok((total_count, stats));
        }

        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path();

        if path.extension().map(|e| e == "meta").unwrap_or(false) {
            continue;
        }

        let asset_type = classify_file(&path);
        if asset_type == "unknown" {
            continue;
        }

        let relative_path = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string();

        let metadata = match fs::metadata(&path) {
            Ok(m) => m,
            Err(_) => continue,
        };

        let size_bytes = metadata.len() as i64;

        let modified_time = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        stats.total_files += 1;

        // Check if file is unchanged (same modified_time and size_bytes)
        if let Some(existing) = existing_assets {
            if let Some((_, existing_mtime, existing_size)) = existing.get(&relative_path) {
                if *existing_mtime == modified_time && *existing_size == size_bytes {
                    // File unchanged, skip indexing
                    stats.unchanged_skipped += 1;
                    continue;
                }
            }
        }

        // File is new or changed
        stats.new_or_changed += 1;

        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        let extension = path
            .extension()
            .map(|e| e.to_string_lossy().to_string())
            .unwrap_or_default();

        let meta_path = PathBuf::from(format!("{}.meta", path.display()));
        let unity_guid = read_unity_guid(&meta_path);

        // Reuse existing asset ID if the file existed before (but was modified)
        let asset_id = existing_assets
            .and_then(|m| m.get(&relative_path))
            .map(|(id, _, _)| id.clone())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

        let asset = Asset {
            id: asset_id,
            project_id: project_id.to_string(),
            absolute_path: path.to_string_lossy().to_string(),
            relative_path: relative_path.clone(),
            file_name,
            extension,
            asset_type: asset_type.to_string(),
            size_bytes,
            modified_time,
            content_hash: None,
            unity_guid,
            import_type: None,
            thumbnail_path: None,
            created_at: now,
            updated_at: now,
        };

        batch.push(asset);
        total_count += 1;

        if batch.len() >= batch_size {
            let should_continue = callback(std::mem::take(&mut batch), total_count, &relative_path);
            if !should_continue {
                return Ok((total_count, stats));
            }
            batch = Vec::with_capacity(batch_size);
        }
    }

    // Send remaining batch
    if !batch.is_empty() {
        callback(batch, total_count, "");
    }

    Ok((total_count, stats))
}
