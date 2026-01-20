use crate::error::AppResult;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    #[serde(skip)]
    pub path: Option<std::path::PathBuf>,

    pub project_root: Option<String>,
    pub output_folder: Option<String>,
    pub ignore_patterns: Vec<String>,
    pub thumbnail_size: u32,
    pub scan_on_focus: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            path: None,
            project_root: None,
            output_folder: None,
            ignore_patterns: vec![
                "Library/".to_string(),
                "Temp/".to_string(),
                "obj/".to_string(),
                "Logs/".to_string(),
                "UserSettings/".to_string(),
                ".git/".to_string(),
                ".vs/".to_string(),
                "Builds/".to_string(),
                "Build/".to_string(),
            ],
            thumbnail_size: 128,
            scan_on_focus: true,
        }
    }
}

impl Settings {
    pub fn load(path: &Path) -> AppResult<Self> {
        if path.exists() {
            let content = std::fs::read_to_string(path)?;
            let mut settings: Settings = serde_json::from_str(&content)?;
            settings.path = Some(path.to_path_buf());
            Ok(settings)
        } else {
            let mut settings = Settings::default();
            settings.path = Some(path.to_path_buf());
            settings.save()?;
            Ok(settings)
        }
    }

    pub fn save(&self) -> AppResult<()> {
        if let Some(path) = &self.path {
            let content = serde_json::to_string_pretty(self)?;
            std::fs::write(path, content)?;
        }
        Ok(())
    }
}
