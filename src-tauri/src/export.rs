use crate::db::{Asset, Database};
use crate::deps::DependencyResolver;
use crate::error::AppResult;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Serialize, Deserialize)]
pub struct ExportManifest {
    pub version: String,
    pub exported_at: String,
    pub source_project: String,
    pub root_asset: String,
    pub assets: Vec<ExportedAsset>,
    pub dependency_graph: Vec<DependencyEdge>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExportedAsset {
    pub relative_path: String,
    pub asset_type: String,
    pub unity_guid: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DependencyEdge {
    pub from: String,
    pub to: String,
    pub relation_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExportResult {
    pub success: bool,
    pub exported_files: Vec<String>,
    pub manifest_path: Option<String>,
    pub error: Option<String>,
}

pub struct Exporter {
    db: Arc<Database>,
    dep_resolver: Arc<DependencyResolver>,
}

impl Exporter {
    pub fn new(db: Arc<Database>) -> Self {
        let dep_resolver = Arc::new(DependencyResolver::new(Arc::clone(&db)));
        Self { db, dep_resolver }
    }

    pub fn export_file(&self, asset: &Asset, dest_folder: &Path) -> AppResult<ExportResult> {
        let source_path = Path::new(&asset.absolute_path);

        if !source_path.exists() {
            return Ok(ExportResult {
                success: false,
                exported_files: vec![],
                manifest_path: None,
                error: Some(format!("Source file not found: {}", asset.absolute_path)),
            });
        }

        // Preserve relative path structure
        let dest_path = dest_folder.join(&asset.relative_path);

        // Create parent directories
        if let Some(parent) = dest_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Copy file
        fs::copy(source_path, &dest_path)?;

        // Also copy .meta file if it exists
        let meta_source = PathBuf::from(format!("{}.meta", asset.absolute_path));
        if meta_source.exists() {
            let meta_dest = PathBuf::from(format!("{}.meta", dest_path.display()));
            fs::copy(&meta_source, &meta_dest)?;
        }

        Ok(ExportResult {
            success: true,
            exported_files: vec![asset.relative_path.clone()],
            manifest_path: None,
            error: None,
        })
    }

    pub fn export_bundle(
        &self,
        asset: &Asset,
        dest_folder: &Path,
        max_depth: usize,
    ) -> AppResult<ExportResult> {
        let mut exported_files = Vec::new();
        let mut exported_paths = HashSet::new();
        let mut dependency_edges = Vec::new();

        // Get all dependencies recursively
        let dep_ids = self.dep_resolver.get_dependency_tree(&asset.id, max_depth)?;

        // Collect all assets to export (root + dependencies)
        let mut assets_to_export = vec![asset.clone()];

        for dep_id in &dep_ids {
            if let Some(dep_asset) = self.db.get_asset(dep_id)? {
                assets_to_export.push(dep_asset);
            }
        }

        // Export each asset
        for export_asset in &assets_to_export {
            let source_path = Path::new(&export_asset.absolute_path);

            if !source_path.exists() {
                tracing::warn!("Skipping missing file: {}", export_asset.absolute_path);
                continue;
            }

            if exported_paths.contains(&export_asset.relative_path) {
                continue;
            }

            let dest_path = dest_folder.join(&export_asset.relative_path);

            if let Some(parent) = dest_path.parent() {
                fs::create_dir_all(parent)?;
            }

            fs::copy(source_path, &dest_path)?;

            // Copy .meta file if exists
            let meta_source = PathBuf::from(format!("{}.meta", export_asset.absolute_path));
            if meta_source.exists() {
                let meta_dest = PathBuf::from(format!("{}.meta", dest_path.display()));
                let _ = fs::copy(&meta_source, &meta_dest);
            }

            exported_files.push(export_asset.relative_path.clone());
            exported_paths.insert(export_asset.relative_path.clone());
        }

        // Build dependency graph for manifest
        for export_asset in &assets_to_export {
            let deps = self.db.get_dependencies(&export_asset.id)?;

            for dep in deps {
                if let Some(to_asset_id) = dep.to_asset_id {
                    if let Some(to_asset) = self.db.get_asset(&to_asset_id)? {
                        if exported_paths.contains(&to_asset.relative_path) {
                            dependency_edges.push(DependencyEdge {
                                from: export_asset.relative_path.clone(),
                                to: to_asset.relative_path.clone(),
                                relation_type: dep.relation_type,
                            });
                        }
                    }
                }
            }
        }

        // Get project info for manifest
        let project = self.db.get_project_by_path(
            Path::new(&asset.absolute_path)
                .parent()
                .and_then(|p| {
                    // Walk up to find project root
                    let mut current = p;
                    while !current.join("Assets").exists() {
                        current = current.parent()?;
                    }
                    Some(current)
                })
                .map(|p| p.to_string_lossy().to_string())
                .as_deref()
                .unwrap_or(""),
        )?;

        // Create manifest
        let manifest = ExportManifest {
            version: "1.0".to_string(),
            exported_at: chrono::Utc::now().to_rfc3339(),
            source_project: project
                .map(|p| p.name)
                .unwrap_or_else(|| "Unknown".to_string()),
            root_asset: asset.relative_path.clone(),
            assets: assets_to_export
                .iter()
                .filter(|a| exported_paths.contains(&a.relative_path))
                .map(|a| ExportedAsset {
                    relative_path: a.relative_path.clone(),
                    asset_type: a.asset_type.clone(),
                    unity_guid: a.unity_guid.clone(),
                })
                .collect(),
            dependency_graph: dependency_edges,
        };

        // Write manifest
        let manifest_path = dest_folder.join("manifest.json");
        let manifest_json = serde_json::to_string_pretty(&manifest)?;
        fs::write(&manifest_path, manifest_json)?;

        Ok(ExportResult {
            success: true,
            exported_files,
            manifest_path: Some(manifest_path.to_string_lossy().to_string()),
            error: None,
        })
    }
}
