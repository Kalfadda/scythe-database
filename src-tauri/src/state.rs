use crate::db::Database;
use crate::error::{AppError, AppResult};
use crate::settings::Settings;
use parking_lot::RwLock;
use std::sync::Arc;
use tauri::{AppHandle, Manager};

pub struct AppState {
    pub db: Arc<Database>,
    pub settings: Arc<RwLock<Settings>>,
    pub app_handle: AppHandle,
}

impl AppState {
    pub fn new(app_handle: AppHandle) -> AppResult<Self> {
        let app_data_dir = app_handle
            .path()
            .app_data_dir()
            .map_err(|e| AppError::Custom(format!("Failed to get app data dir: {}", e)))?;

        std::fs::create_dir_all(&app_data_dir)?;

        let db_path = app_data_dir.join("scythe.db");
        let db = Database::new(&db_path)?;

        let settings_path = app_data_dir.join("settings.json");
        let settings = Settings::load(&settings_path)?;

        Ok(Self {
            db: Arc::new(db),
            settings: Arc::new(RwLock::new(settings)),
            app_handle,
        })
    }

    pub fn thumbnail_dir(&self) -> AppResult<std::path::PathBuf> {
        let app_data_dir = self
            .app_handle
            .path()
            .app_data_dir()
            .map_err(|e| AppError::Custom(format!("Failed to get app data dir: {}", e)))?;

        let thumb_dir = app_data_dir.join("thumbnails");
        std::fs::create_dir_all(&thumb_dir)?;
        Ok(thumb_dir)
    }
}
