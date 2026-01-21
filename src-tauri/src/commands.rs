use crate::db::{Asset, Dependency, Project, TypeCount};
use crate::deps::DependencyResolver;
use crate::error::AppError;
use crate::export::{ExportResult, Exporter};
use crate::indexer::Indexer;
use crate::previews::{parse_material_file, parse_model_info, MaterialInfo, ModelInfo, PreviewGenerator};
use crate::scanner::{count_scannable_files, scan_files_batch};
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use tauri::{Emitter, State};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanProgress {
    pub scanned: usize,
    pub total: Option<usize>,
    pub current_path: String,
    pub phase: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AssetsResponse {
    pub assets: Vec<Asset>,
    pub total: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SettingsResponse {
    pub project_root: Option<String>,
    pub output_folder: Option<String>,
}

#[tauri::command]
pub async fn set_project_root(
    path: String,
    state: State<'_, AppState>,
) -> Result<Project, AppError> {
    let root = Path::new(&path);

    // Accept any valid folder, not just Unity projects
    if !root.is_dir() {
        return Err(AppError::InvalidProject(
            "Not a valid folder.".to_string(),
        ));
    }

    // Get folder name
    let name = root
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "Unknown Folder".to_string());

    // Create or get project in database
    let project = state.db.get_or_create_project(&path, &name)?;

    // Save to settings
    {
        let mut settings = state.settings.write();
        settings.project_root = Some(path);
        settings.save()?;
    }

    Ok(project)
}

#[tauri::command]
pub async fn set_output_folder(
    path: String,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    let mut settings = state.settings.write();
    settings.output_folder = Some(path);
    settings.save()?;
    Ok(())
}

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<SettingsResponse, AppError> {
    let settings = state.settings.read();
    Ok(SettingsResponse {
        project_root: settings.project_root.clone(),
        output_folder: settings.output_folder.clone(),
    })
}

#[tauri::command]
pub async fn get_current_project(state: State<'_, AppState>) -> Result<Option<Project>, AppError> {
    let settings = state.settings.read();

    if let Some(root) = &settings.project_root {
        return state.db.get_project_by_path(root);
    }

    Ok(None)
}

#[tauri::command]
pub async fn start_scan(
    project_id: String,
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    let db = Arc::clone(&state.db);
    let settings = state.settings.read().clone();
    let cancel_flag = Arc::clone(&state.cancel_flag);

    // Reset cancel flag at start
    state.reset_cancel();

    let project = state
        .db
        .get_project_by_path(settings.project_root.as_deref().unwrap_or(""))?
        .ok_or_else(|| AppError::Custom("Project not found".to_string()))?;

    let root_path = project.root_path.clone();
    let project_id_clone = project_id.clone();
    let ignore_patterns = settings.ignore_patterns.clone();

    // Spawn scanning task
    tokio::task::spawn_blocking(move || {
        let db_clone = Arc::clone(&db);
        let indexer = Indexer::new(Arc::clone(&db));
        let mut last_refresh = std::time::Instant::now();

        // Phase 0: Count files first for accurate progress
        let _ = app_handle.emit(
            "scan-progress",
            ScanProgress {
                scanned: 0,
                total: None,
                current_path: "".to_string(),
                phase: "counting".to_string(),
            },
        );

        let cancel_flag_count = Arc::clone(&cancel_flag);
        let total_files = match count_scannable_files(
            Path::new(&root_path),
            &ignore_patterns,
            cancel_flag_count,
            |count| {
                let _ = app_handle.emit(
                    "scan-progress",
                    ScanProgress {
                        scanned: count,
                        total: None,
                        current_path: "".to_string(),
                        phase: "counting".to_string(),
                    },
                );
            },
        ) {
            Ok(count) => count,
            Err(e) => {
                tracing::error!("Failed to count files: {}", e);
                0
            }
        };

        // Check if cancelled during counting
        if cancel_flag.load(std::sync::atomic::Ordering::SeqCst) {
            let _ = app_handle.emit(
                "scan-progress",
                ScanProgress {
                    scanned: 0,
                    total: None,
                    current_path: "".to_string(),
                    phase: "cancelled".to_string(),
                },
            );
            return;
        }

        // Phase 1: Scan and index files
        let _ = app_handle.emit(
            "scan-progress",
            ScanProgress {
                scanned: 0,
                total: Some(total_files),
                current_path: "".to_string(),
                phase: "indexing".to_string(),
            },
        );

        let cancel_flag_scan = Arc::clone(&cancel_flag);
        let total = scan_files_batch(
            Path::new(&root_path),
            &project_id_clone,
            &ignore_patterns,
            25,
            cancel_flag_scan,
            |batch, count, current_path| {
                // Index the batch
                if let Err(e) = indexer.upsert_batch(&batch) {
                    tracing::error!("Failed to index batch: {}", e);
                }

                let _ = app_handle.emit(
                    "scan-progress",
                    ScanProgress {
                        scanned: count,
                        total: Some(total_files),
                        current_path: current_path.to_string(),
                        phase: "indexing".to_string(),
                    },
                );

                // Signal frontend to refresh every 200ms
                if last_refresh.elapsed().as_millis() > 200 {
                    let _ = app_handle.emit("assets-updated", count);
                    last_refresh = std::time::Instant::now();
                }

                // Return true to continue, false to stop
                !cancel_flag.load(std::sync::atomic::Ordering::SeqCst)
            },
        );

        // Check if cancelled during indexing
        if cancel_flag.load(std::sync::atomic::Ordering::SeqCst) {
            let _ = app_handle.emit(
                "scan-progress",
                ScanProgress {
                    scanned: 0,
                    total: None,
                    current_path: "".to_string(),
                    phase: "cancelled".to_string(),
                },
            );
            return;
        }

        let file_count = total.unwrap_or(0) as i64;

        // Signal final asset update
        let _ = app_handle.emit("assets-updated", file_count);

        // Phase 2: Resolve dependencies with progress
        let _ = app_handle.emit(
            "scan-progress",
            ScanProgress {
                scanned: 0,
                total: None,
                current_path: "".to_string(),
                phase: "dependencies".to_string(),
            },
        );

        let dep_resolver = DependencyResolver::new(Arc::clone(&db_clone));
        let cancel_flag_deps = Arc::clone(&cancel_flag);
        let app_handle_deps = app_handle.clone();
        if let Err(e) = dep_resolver.resolve_all_for_project_with_progress(
            &project_id_clone,
            cancel_flag_deps,
            |processed, total| {
                let _ = app_handle_deps.emit(
                    "scan-progress",
                    ScanProgress {
                        scanned: processed,
                        total: Some(total),
                        current_path: "".to_string(),
                        phase: "dependencies".to_string(),
                    },
                );
            },
        ) {
            tracing::error!("Failed to resolve dependencies: {}", e);
        }

        // Check if cancelled during dependencies
        if cancel_flag.load(std::sync::atomic::Ordering::SeqCst) {
            let _ = app_handle.emit(
                "scan-progress",
                ScanProgress {
                    scanned: 0,
                    total: None,
                    current_path: "".to_string(),
                    phase: "cancelled".to_string(),
                },
            );
            return;
        }

        // Update project scan time
        if let Err(e) = db_clone.update_project_scan_time(&project_id_clone, file_count) {
            tracing::error!("Failed to update project scan time: {}", e);
        }

        // Complete scan phase - thumbnails will be generated separately via regenerate_thumbnails
        let _ = app_handle.emit(
            "scan-progress",
            ScanProgress {
                scanned: file_count as usize,
                total: Some(file_count as usize),
                current_path: "".to_string(),
                phase: "complete".to_string(),
            },
        );
    });

    Ok(())
}

#[tauri::command]
pub async fn cancel_operation(state: State<'_, AppState>) -> Result<(), AppError> {
    state.request_cancel();
    Ok(())
}

#[tauri::command]
pub async fn get_assets(
    project_id: String,
    search_query: Option<String>,
    asset_types: Option<Vec<String>>,
    page: i64,
    page_size: i64,
    state: State<'_, AppState>,
) -> Result<AssetsResponse, AppError> {
    let (assets, total) = state.db.get_assets(
        &project_id,
        search_query.as_deref(),
        asset_types.as_deref(),
        page,
        page_size,
    )?;

    Ok(AssetsResponse { assets, total })
}

#[tauri::command]
pub async fn get_asset(id: String, state: State<'_, AppState>) -> Result<Asset, AppError> {
    state
        .db
        .get_asset(&id)?
        .ok_or_else(|| AppError::AssetNotFound(id))
}

#[tauri::command]
pub async fn get_dependencies(
    asset_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<Dependency>, AppError> {
    state.db.get_dependencies(&asset_id)
}

#[tauri::command]
pub async fn get_dependents(
    asset_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<Dependency>, AppError> {
    state.db.get_dependents(&asset_id)
}

#[tauri::command]
pub async fn get_type_counts(
    project_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<TypeCount>, AppError> {
    state.db.get_type_counts(&project_id)
}

#[tauri::command]
pub async fn export_file(
    asset_id: String,
    dest_folder: String,
    state: State<'_, AppState>,
) -> Result<ExportResult, AppError> {
    let asset = state
        .db
        .get_asset(&asset_id)?
        .ok_or_else(|| AppError::AssetNotFound(asset_id))?;

    let exporter = Exporter::new(Arc::clone(&state.db));
    exporter.export_file(&asset, Path::new(&dest_folder))
}

#[tauri::command]
pub async fn export_bundle(
    asset_id: String,
    dest_folder: String,
    state: State<'_, AppState>,
) -> Result<ExportResult, AppError> {
    let asset = state
        .db
        .get_asset(&asset_id)?
        .ok_or_else(|| AppError::AssetNotFound(asset_id))?;

    let exporter = Exporter::new(Arc::clone(&state.db));
    exporter.export_bundle(&asset, Path::new(&dest_folder), 5)
}

#[tauri::command]
pub async fn reveal_in_explorer(path: String) -> Result<(), AppError> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .args(["/select,", &path])
            .spawn()
            .map_err(|e| AppError::Io(e))?;
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .args(["-R", &path])
            .spawn()
            .map_err(|e| AppError::Io(e))?;
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(Path::new(&path).parent().unwrap_or(Path::new(&path)))
            .spawn()
            .map_err(|e| AppError::Io(e))?;
    }

    Ok(())
}

#[tauri::command]
pub async fn get_material_info(
    asset_id: String,
    state: State<'_, AppState>,
) -> Result<Option<MaterialInfo>, AppError> {
    let asset = state
        .db
        .get_asset(&asset_id)?
        .ok_or_else(|| AppError::AssetNotFound(asset_id))?;

    if asset.asset_type != "material" {
        return Ok(None);
    }

    Ok(parse_material_file(Path::new(&asset.absolute_path)))
}

#[tauri::command]
pub async fn get_model_info(
    asset_id: String,
    state: State<'_, AppState>,
) -> Result<Option<ModelInfo>, AppError> {
    let asset = state
        .db
        .get_asset(&asset_id)?
        .ok_or_else(|| AppError::AssetNotFound(asset_id))?;

    if asset.asset_type != "model" {
        return Ok(None);
    }

    Ok(parse_model_info(Path::new(&asset.absolute_path)))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundlePreview {
    pub root_asset: BundleAssetInfo,
    pub dependencies: Vec<BundleAssetInfo>,
    pub total_size_bytes: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleAssetInfo {
    pub id: String,
    pub file_name: String,
    pub relative_path: String,
    pub asset_type: String,
    pub size_bytes: i64,
}

#[tauri::command]
pub async fn get_bundle_preview(
    asset_id: String,
    state: State<'_, AppState>,
) -> Result<BundlePreview, AppError> {
    let asset = state
        .db
        .get_asset(&asset_id)?
        .ok_or_else(|| AppError::AssetNotFound(asset_id.clone()))?;

    let dep_resolver = DependencyResolver::new(Arc::clone(&state.db));
    let dep_ids = dep_resolver.get_dependency_tree(&asset_id, 5)?;

    let mut dependencies = Vec::new();
    let mut total_size = asset.size_bytes;

    for dep_id in dep_ids {
        if let Some(dep_asset) = state.db.get_asset(&dep_id)? {
            total_size += dep_asset.size_bytes;
            dependencies.push(BundleAssetInfo {
                id: dep_asset.id,
                file_name: dep_asset.file_name,
                relative_path: dep_asset.relative_path,
                asset_type: dep_asset.asset_type,
                size_bytes: dep_asset.size_bytes,
            });
        }
    }

    Ok(BundlePreview {
        root_asset: BundleAssetInfo {
            id: asset.id,
            file_name: asset.file_name,
            relative_path: asset.relative_path,
            asset_type: asset.asset_type,
            size_bytes: asset.size_bytes,
        },
        dependencies,
        total_size_bytes: total_size,
    })
}

#[tauri::command]
pub async fn get_thumbnail_base64(
    asset_id: String,
    state: State<'_, AppState>,
) -> Result<Option<String>, AppError> {
    let asset = state
        .db
        .get_asset(&asset_id)?
        .ok_or_else(|| AppError::AssetNotFound(asset_id))?;

    // First try thumbnail path
    if let Some(thumb_path) = &asset.thumbnail_path {
        // Check for special markers
        if thumb_path == "TOO_LARGE" {
            return Ok(Some("TOO_LARGE".to_string()));
        }
        if thumb_path == "UNSUPPORTED" {
            return Ok(Some("UNSUPPORTED".to_string()));
        }

        if let Ok(data) = std::fs::read(thumb_path) {
            let base64 = base64_encode(&data);
            let ext = Path::new(thumb_path)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("png");
            let mime = match ext {
                "jpg" | "jpeg" => "image/jpeg",
                "png" => "image/png",
                "gif" => "image/gif",
                _ => "image/png",
            };
            return Ok(Some(format!("data:{};base64,{}", mime, base64)));
        }
    }

    // For textures, try to load and resize the original
    if asset.asset_type == "texture" {
        let source_path = Path::new(&asset.absolute_path);
        let ext = source_path
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_lowercase())
            .unwrap_or_default();

        // Check supported formats
        match ext.as_str() {
            "png" | "jpg" | "jpeg" | "tga" | "bmp" | "gif" => {
                if let Ok(img) = image::open(source_path) {
                    let thumb = img.thumbnail(128, 128);
                    let mut buf = std::io::Cursor::new(Vec::new());
                    if thumb.write_to(&mut buf, image::ImageFormat::Png).is_ok() {
                        let base64 = base64_encode(buf.get_ref());
                        return Ok(Some(format!("data:image/png;base64,{}", base64)));
                    }
                }
            }
            "psd" => {
                // Handle Photoshop files - wrap in catch_unwind since psd crate can panic
                if let Ok(psd_data) = std::fs::read(source_path) {
                    use std::panic::AssertUnwindSafe;
                    let psd_data_ref = &psd_data;
                    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                        psd::Psd::from_bytes(psd_data_ref).ok().and_then(|psd| {
                            let rgba = psd.rgba();
                            let width = psd.width();
                            let height = psd.height();
                            image::RgbaImage::from_raw(width, height, rgba)
                        })
                    }));

                    if let Ok(Some(img)) = result {
                        let dyn_img = image::DynamicImage::ImageRgba8(img);
                        let thumb = dyn_img.thumbnail(128, 128);
                        let mut buf = std::io::Cursor::new(Vec::new());
                        if thumb.write_to(&mut buf, image::ImageFormat::Png).is_ok() {
                            let base64 = base64_encode(buf.get_ref());
                            return Ok(Some(format!("data:image/png;base64,{}", base64)));
                        }
                    }
                }
            }
            _ => {}
        }
    }

    Ok(None)
}

fn base64_encode(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(data)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThumbnailProgress {
    pub generated: usize,
    pub total: usize,
    pub phase: String, // "counting", "generating", "complete"
}

#[tauri::command]
pub async fn regenerate_thumbnails(
    project_id: String,
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    let db = Arc::clone(&state.db);
    let thumb_dir = state.thumbnail_dir()?;
    let cancel_flag = Arc::clone(&state.cancel_flag);

    // Spawn thumbnail generation task
    tokio::task::spawn_blocking(move || {
        // Phase 1: Clear existing thumbnails and count assets
        let _ = app_handle.emit(
            "thumbnail-progress",
            ThumbnailProgress {
                generated: 0,
                total: 0,
                phase: "counting".to_string(),
            },
        );

        // Check cancellation
        if cancel_flag.load(std::sync::atomic::Ordering::SeqCst) {
            let _ = app_handle.emit(
                "thumbnail-progress",
                ThumbnailProgress {
                    generated: 0,
                    total: 0,
                    phase: "cancelled".to_string(),
                },
            );
            return;
        }

        // Clear existing thumbnail paths to force regeneration
        if let Err(e) = db.clear_thumbnail_paths(&project_id) {
            tracing::error!("Failed to clear thumbnail paths: {}", e);
        }

        // Get count of texture and material assets
        let total = match db.count_thumbnail_assets(&project_id) {
            Ok(count) => count,
            Err(e) => {
                tracing::error!("Failed to count assets: {}", e);
                return;
            }
        };

        if total == 0 {
            let _ = app_handle.emit(
                "thumbnail-progress",
                ThumbnailProgress {
                    generated: 0,
                    total: 0,
                    phase: "complete".to_string(),
                },
            );
            return;
        }

        // Phase 2: Generate thumbnails in batches
        let preview_gen = PreviewGenerator::new(Arc::clone(&db), thumb_dir, 128);
        let mut generated = 0usize;
        let batch_size = 25;

        loop {
            // Check cancellation
            if cancel_flag.load(std::sync::atomic::Ordering::SeqCst) {
                let _ = app_handle.emit(
                    "thumbnail-progress",
                    ThumbnailProgress {
                        generated,
                        total,
                        phase: "cancelled".to_string(),
                    },
                );
                return;
            }

            let _ = app_handle.emit(
                "thumbnail-progress",
                ThumbnailProgress {
                    generated,
                    total,
                    phase: "generating".to_string(),
                },
            );

            match preview_gen.generate_thumbnails_for_project(&project_id, batch_size) {
                Ok(0) => break, // No more thumbnails to generate
                Ok(count) => {
                    generated += count;
                    let _ = app_handle.emit("assets-updated", 0);
                }
                Err(e) => {
                    tracing::error!("Thumbnail generation error: {}", e);
                    break;
                }
            }
        }

        // Phase 3: Complete
        let _ = app_handle.emit(
            "thumbnail-progress",
            ThumbnailProgress {
                generated,
                total,
                phase: "complete".to_string(),
            },
        );
    });

    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelAssetInfo {
    pub id: String,
    pub absolute_path: String,
    pub extension: String,
    pub modified_time: i64,
}

#[tauri::command]
pub async fn get_model_assets_for_thumbnails(
    project_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<ModelAssetInfo>, AppError> {
    let assets = state.db.get_model_assets(&project_id)?;

    Ok(assets
        .into_iter()
        .map(|a| ModelAssetInfo {
            id: a.id,
            absolute_path: a.absolute_path,
            extension: a.extension,
            modified_time: a.modified_time,
        })
        .collect())
}
