use crate::db::Asset;
use crate::error::{AppError, AppResult};
use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub struct Scanner {
    ignore_patterns: Vec<String>,
}

impl Scanner {
    pub fn new(ignore_patterns: Vec<String>) -> Self {
        Self { ignore_patterns }
    }

    pub fn is_valid_unity_project(root: &Path) -> bool {
        root.join("Assets").is_dir() && root.join("ProjectSettings").is_dir()
    }

    #[allow(dead_code)]
    pub fn scan(&self, root: &Path, project_id: &str) -> AppResult<Vec<Asset>> {
        if !Self::is_valid_unity_project(root) {
            return Err(AppError::InvalidProject(
                "Not a valid Unity project (missing Assets or ProjectSettings folder)".to_string(),
            ));
        }

        let mut assets = Vec::new();
        let now = chrono::Utc::now().timestamp();

        for entry in WalkDir::new(root)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| !self.should_ignore(e.path(), root))
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

            let asset_type = classify_file(path);
            if asset_type == "unknown" {
                continue;
            }

            let relative_path = path
                .strip_prefix(root)
                .unwrap_or(path)
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

            let metadata = fs::metadata(path)?;
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
        let relative = path.strip_prefix(root).unwrap_or(path);
        let relative_str = relative.to_string_lossy();

        for pattern in &self.ignore_patterns {
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

pub fn scan_files_batch(
    root: &Path,
    project_id: &str,
    ignore_patterns: &[String],
    batch_size: usize,
    mut callback: impl FnMut(Vec<Asset>, usize, &str),
) -> AppResult<usize> {
    let scanner = Scanner::new(ignore_patterns.to_vec());

    if !Scanner::is_valid_unity_project(root) {
        return Err(AppError::InvalidProject(
            "Not a valid Unity project (missing Assets or ProjectSettings folder)".to_string(),
        ));
    }

    let mut batch = Vec::with_capacity(batch_size);
    let mut total_count = 0;
    let now = chrono::Utc::now().timestamp();

    for entry in WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| !scanner.should_ignore(e.path(), root))
    {
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

        let asset_type = classify_file(path);
        if asset_type == "unknown" {
            continue;
        }

        let relative_path = path
            .strip_prefix(root)
            .unwrap_or(path)
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

        let metadata = match fs::metadata(path) {
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

        let meta_path = PathBuf::from(format!("{}.meta", path.display()));
        let unity_guid = read_unity_guid(&meta_path);

        let asset = Asset {
            id: uuid::Uuid::new_v4().to_string(),
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
            callback(std::mem::take(&mut batch), total_count, &relative_path);
            batch = Vec::with_capacity(batch_size);
        }
    }

    // Send remaining batch
    if !batch.is_empty() {
        callback(batch, total_count, "");
    }

    Ok(total_count)
}
