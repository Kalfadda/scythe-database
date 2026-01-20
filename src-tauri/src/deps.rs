use crate::db::{Asset, Database, Dependency};
use crate::error::AppResult;
use regex::Regex;
use std::collections::HashSet;
use std::fs;
use std::sync::Arc;

pub struct DependencyResolver {
    db: Arc<Database>,
    guid_regex: Regex,
}

impl DependencyResolver {
    pub fn new(db: Arc<Database>) -> Self {
        Self {
            db,
            guid_regex: Regex::new(r"guid:\s*([a-f0-9]{32})").unwrap(),
        }
    }

    pub fn resolve_dependencies_for_asset(&self, asset: &Asset) -> AppResult<Vec<Dependency>> {
        // Only parse certain asset types
        match asset.asset_type.as_str() {
            "material" | "prefab" | "scene" | "scriptable_object" => {}
            _ => return Ok(Vec::new()),
        }

        // Read file content
        let content = match fs::read_to_string(&asset.absolute_path) {
            Ok(c) => c,
            Err(_) => return Ok(Vec::new()),
        };

        // Extract GUIDs from content
        let guids = self.extract_guids(&content);

        // Filter out self-reference and create dependencies
        let mut dependencies = Vec::new();
        let now = chrono::Utc::now().timestamp();

        for guid in guids {
            // Skip self-reference
            if asset.unity_guid.as_ref() == Some(&guid) {
                continue;
            }

            // Try to resolve the GUID to an asset
            let to_asset = self.db.get_asset_by_guid(&asset.project_id, &guid)?;

            let relation_type = self.infer_relation_type(&asset.asset_type, &to_asset);

            let dep = Dependency {
                id: uuid::Uuid::new_v4().to_string(),
                from_asset_id: asset.id.clone(),
                to_asset_id: to_asset.map(|a| a.id),
                to_guid: guid,
                relation_type,
                confidence: "high".to_string(),
                created_at: now,
            };

            dependencies.push(dep);
        }

        Ok(dependencies)
    }

    fn extract_guids(&self, content: &str) -> Vec<String> {
        let mut guids = HashSet::new();

        for cap in self.guid_regex.captures_iter(content) {
            if let Some(m) = cap.get(1) {
                guids.insert(m.as_str().to_string());
            }
        }

        guids.into_iter().collect()
    }

    fn infer_relation_type(&self, from_type: &str, to_asset: &Option<Asset>) -> String {
        let to_type = to_asset
            .as_ref()
            .map(|a| a.asset_type.as_str())
            .unwrap_or("unknown");

        match (from_type, to_type) {
            ("material", "texture") => "material_texture".to_string(),
            ("material", "shader") => "material_shader".to_string(),
            ("prefab", "material") => "prefab_material".to_string(),
            ("prefab", "model") => "prefab_model".to_string(),
            ("prefab", "prefab") => "prefab_prefab".to_string(),
            ("prefab", "texture") => "prefab_texture".to_string(),
            ("scene", "prefab") => "scene_prefab".to_string(),
            ("scene", "material") => "scene_material".to_string(),
            ("scene", _) => "scene_reference".to_string(),
            _ => "reference".to_string(),
        }
    }

    pub fn resolve_all_for_project(&self, project_id: &str) -> AppResult<usize> {
        let assets = self.db.get_parseable_assets(project_id)?;
        let mut total_deps = 0;

        for asset in assets {
            // Clear existing dependencies for this asset
            self.db.delete_dependencies_for_asset(&asset.id)?;

            // Resolve new dependencies
            let deps = self.resolve_dependencies_for_asset(&asset)?;

            for dep in &deps {
                self.db.insert_dependency(dep)?;
            }

            total_deps += deps.len();
        }

        Ok(total_deps)
    }

    pub fn get_dependency_tree(
        &self,
        asset_id: &str,
        max_depth: usize,
    ) -> AppResult<Vec<String>> {
        let mut visited = HashSet::new();
        let mut result = Vec::new();

        self.collect_dependencies(asset_id, 0, max_depth, &mut visited, &mut result)?;

        Ok(result)
    }

    fn collect_dependencies(
        &self,
        asset_id: &str,
        depth: usize,
        max_depth: usize,
        visited: &mut HashSet<String>,
        result: &mut Vec<String>,
    ) -> AppResult<()> {
        if depth >= max_depth || visited.contains(asset_id) {
            return Ok(());
        }

        visited.insert(asset_id.to_string());

        let deps = self.db.get_dependencies(asset_id)?;

        for dep in deps {
            if let Some(to_id) = dep.to_asset_id {
                if !visited.contains(&to_id) {
                    result.push(to_id.clone());
                    self.collect_dependencies(&to_id, depth + 1, max_depth, visited, result)?;
                }
            }
        }

        Ok(())
    }
}
