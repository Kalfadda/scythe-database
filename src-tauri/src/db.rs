use crate::error::AppResult;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::params;
use std::path::Path;

pub type DbPool = Pool<SqliteConnectionManager>;

pub struct Database {
    pool: DbPool,
}

impl Database {
    pub fn new(path: &Path) -> AppResult<Self> {
        let manager = SqliteConnectionManager::file(path);
        let pool = Pool::builder().max_size(4).build(manager)?;

        // Enable WAL mode for better concurrent performance
        {
            let conn = pool.get()?;
            conn.execute_batch(
                r#"
                PRAGMA journal_mode = WAL;
                PRAGMA synchronous = NORMAL;
                PRAGMA cache_size = -64000;
                PRAGMA temp_store = MEMORY;
                "#,
            )?;
        }

        let db = Self { pool };
        db.init_schema()?;
        Ok(db)
    }

    pub fn pool(&self) -> &DbPool {
        &self.pool
    }

    fn init_schema(&self) -> AppResult<()> {
        let conn = self.pool.get()?;

        conn.execute_batch(
            r#"
            -- Projects table
            CREATE TABLE IF NOT EXISTS projects (
                id TEXT PRIMARY KEY,
                root_path TEXT NOT NULL UNIQUE,
                name TEXT NOT NULL,
                last_scan_time INTEGER,
                file_count INTEGER DEFAULT 0,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );

            -- Main assets table
            CREATE TABLE IF NOT EXISTS assets (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                absolute_path TEXT NOT NULL,
                relative_path TEXT NOT NULL,
                file_name TEXT NOT NULL,
                extension TEXT NOT NULL,
                asset_type TEXT NOT NULL,
                size_bytes INTEGER NOT NULL,
                modified_time INTEGER NOT NULL,
                content_hash TEXT,
                unity_guid TEXT,
                import_type TEXT,
                thumbnail_path TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
                UNIQUE(project_id, relative_path)
            );

            CREATE INDEX IF NOT EXISTS idx_assets_project ON assets(project_id);
            CREATE INDEX IF NOT EXISTS idx_assets_type ON assets(asset_type);
            CREATE INDEX IF NOT EXISTS idx_assets_guid ON assets(unity_guid);
            CREATE INDEX IF NOT EXISTS idx_assets_relative_path ON assets(relative_path);
            CREATE INDEX IF NOT EXISTS idx_assets_project_type ON assets(project_id, asset_type);

            -- Full-text search virtual table
            CREATE VIRTUAL TABLE IF NOT EXISTS assets_fts USING fts5(
                file_name,
                relative_path,
                content=assets,
                content_rowid=rowid
            );

            -- Dependencies table
            CREATE TABLE IF NOT EXISTS dependencies (
                id TEXT PRIMARY KEY,
                from_asset_id TEXT NOT NULL,
                to_asset_id TEXT,
                to_guid TEXT,
                relation_type TEXT NOT NULL,
                confidence TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                FOREIGN KEY (from_asset_id) REFERENCES assets(id) ON DELETE CASCADE,
                FOREIGN KEY (to_asset_id) REFERENCES assets(id) ON DELETE SET NULL
            );

            CREATE INDEX IF NOT EXISTS idx_deps_from ON dependencies(from_asset_id);
            CREATE INDEX IF NOT EXISTS idx_deps_to ON dependencies(to_asset_id);
            CREATE INDEX IF NOT EXISTS idx_deps_guid ON dependencies(to_guid);

            -- Preview cache tracking
            CREATE TABLE IF NOT EXISTS preview_cache (
                asset_id TEXT PRIMARY KEY,
                thumb_path TEXT NOT NULL,
                thumb_size INTEGER NOT NULL,
                version_key TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                FOREIGN KEY (asset_id) REFERENCES assets(id) ON DELETE CASCADE
            );
            "#,
        )?;

        // Create FTS triggers if they don't exist (checking first to avoid errors)
        let trigger_exists: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='trigger' AND name='assets_ai')",
            [],
            |row| row.get(0),
        )?;

        if !trigger_exists {
            conn.execute_batch(
                r#"
                CREATE TRIGGER assets_ai AFTER INSERT ON assets BEGIN
                    INSERT INTO assets_fts(rowid, file_name, relative_path)
                    VALUES (NEW.rowid, NEW.file_name, NEW.relative_path);
                END;

                CREATE TRIGGER assets_ad AFTER DELETE ON assets BEGIN
                    INSERT INTO assets_fts(assets_fts, rowid, file_name, relative_path)
                    VALUES ('delete', OLD.rowid, OLD.file_name, OLD.relative_path);
                END;

                CREATE TRIGGER assets_au AFTER UPDATE ON assets BEGIN
                    INSERT INTO assets_fts(assets_fts, rowid, file_name, relative_path)
                    VALUES ('delete', OLD.rowid, OLD.file_name, OLD.relative_path);
                    INSERT INTO assets_fts(rowid, file_name, relative_path)
                    VALUES (NEW.rowid, NEW.file_name, NEW.relative_path);
                END;
                "#,
            )?;
        }

        Ok(())
    }
}

// Data structures for database operations
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub root_path: String,
    pub name: String,
    pub last_scan_time: Option<i64>,
    pub file_count: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Asset {
    pub id: String,
    pub project_id: String,
    pub absolute_path: String,
    pub relative_path: String,
    pub file_name: String,
    pub extension: String,
    pub asset_type: String,
    pub size_bytes: i64,
    pub modified_time: i64,
    pub content_hash: Option<String>,
    pub unity_guid: Option<String>,
    pub import_type: Option<String>,
    pub thumbnail_path: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    pub id: String,
    pub from_asset_id: String,
    pub to_asset_id: Option<String>,
    pub to_guid: String,
    pub relation_type: String,
    pub confidence: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeCount {
    pub asset_type: String,
    pub count: i64,
}

impl Database {
    pub fn get_or_create_project(&self, root_path: &str, name: &str) -> AppResult<Project> {
        let conn = self.pool.get()?;
        let now = chrono::Utc::now().timestamp();

        // Try to find existing project
        let existing: Option<Project> = conn
            .query_row(
                "SELECT id, root_path, name, last_scan_time, file_count, created_at, updated_at
                 FROM projects WHERE root_path = ?1",
                params![root_path],
                |row| {
                    Ok(Project {
                        id: row.get(0)?,
                        root_path: row.get(1)?,
                        name: row.get(2)?,
                        last_scan_time: row.get(3)?,
                        file_count: row.get(4)?,
                        created_at: row.get(5)?,
                        updated_at: row.get(6)?,
                    })
                },
            )
            .ok();

        if let Some(project) = existing {
            return Ok(project);
        }

        // Create new project
        let id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO projects (id, root_path, name, file_count, created_at, updated_at)
             VALUES (?1, ?2, ?3, 0, ?4, ?4)",
            params![id, root_path, name, now],
        )?;

        Ok(Project {
            id,
            root_path: root_path.to_string(),
            name: name.to_string(),
            last_scan_time: None,
            file_count: 0,
            created_at: now,
            updated_at: now,
        })
    }

    pub fn get_project_by_path(&self, root_path: &str) -> AppResult<Option<Project>> {
        let conn = self.pool.get()?;

        let result = conn
            .query_row(
                "SELECT id, root_path, name, last_scan_time, file_count, created_at, updated_at
                 FROM projects WHERE root_path = ?1",
                params![root_path],
                |row| {
                    Ok(Project {
                        id: row.get(0)?,
                        root_path: row.get(1)?,
                        name: row.get(2)?,
                        last_scan_time: row.get(3)?,
                        file_count: row.get(4)?,
                        created_at: row.get(5)?,
                        updated_at: row.get(6)?,
                    })
                },
            )
            .ok();

        Ok(result)
    }

    pub fn update_project_scan_time(&self, project_id: &str, file_count: i64) -> AppResult<()> {
        let conn = self.pool.get()?;
        let now = chrono::Utc::now().timestamp();

        conn.execute(
            "UPDATE projects SET last_scan_time = ?1, file_count = ?2, updated_at = ?1 WHERE id = ?3",
            params![now, file_count, project_id],
        )?;

        Ok(())
    }

    pub fn upsert_asset(&self, asset: &Asset) -> AppResult<()> {
        let conn = self.pool.get()?;

        conn.execute(
            r#"
            INSERT INTO assets (id, project_id, absolute_path, relative_path, file_name, extension,
                               asset_type, size_bytes, modified_time, content_hash, unity_guid,
                               import_type, thumbnail_path, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
            ON CONFLICT(project_id, relative_path) DO UPDATE SET
                absolute_path = excluded.absolute_path,
                file_name = excluded.file_name,
                extension = excluded.extension,
                asset_type = excluded.asset_type,
                size_bytes = excluded.size_bytes,
                modified_time = excluded.modified_time,
                content_hash = excluded.content_hash,
                unity_guid = excluded.unity_guid,
                import_type = excluded.import_type,
                thumbnail_path = excluded.thumbnail_path,
                updated_at = excluded.updated_at
            "#,
            params![
                asset.id,
                asset.project_id,
                asset.absolute_path,
                asset.relative_path,
                asset.file_name,
                asset.extension,
                asset.asset_type,
                asset.size_bytes,
                asset.modified_time,
                asset.content_hash,
                asset.unity_guid,
                asset.import_type,
                asset.thumbnail_path,
                asset.created_at,
                asset.updated_at,
            ],
        )?;

        Ok(())
    }

    pub fn get_assets(
        &self,
        project_id: &str,
        search_query: Option<&str>,
        asset_types: Option<&[String]>,
        page: i64,
        page_size: i64,
    ) -> AppResult<(Vec<Asset>, i64)> {
        let conn = self.pool.get()?;
        let offset = page * page_size;

        let (where_clause, count_where) = if let Some(query) = search_query {
            if query.trim().is_empty() {
                ("WHERE a.project_id = ?1".to_string(), "WHERE project_id = ?1".to_string())
            } else {
                (
                    "WHERE a.project_id = ?1 AND a.rowid IN (SELECT rowid FROM assets_fts WHERE assets_fts MATCH ?4)".to_string(),
                    "WHERE project_id = ?1 AND rowid IN (SELECT rowid FROM assets_fts WHERE assets_fts MATCH ?2)".to_string(),
                )
            }
        } else {
            ("WHERE a.project_id = ?1".to_string(), "WHERE project_id = ?1".to_string())
        };

        let type_filter = if let Some(types) = asset_types {
            if types.is_empty() {
                String::new()
            } else {
                let placeholders: Vec<String> = types.iter().map(|t| format!("'{}'", t)).collect();
                format!(" AND asset_type IN ({})", placeholders.join(", "))
            }
        } else {
            String::new()
        };

        // Get total count
        let count_sql = format!(
            "SELECT COUNT(*) FROM assets {}{}",
            count_where, type_filter
        );

        let total: i64 = if let Some(query) = search_query {
            if query.trim().is_empty() {
                conn.query_row(&count_sql, params![project_id], |row| row.get(0))?
            } else {
                let fts_query = format!("{}*", query);
                conn.query_row(&count_sql, params![project_id, fts_query], |row| row.get(0))?
            }
        } else {
            conn.query_row(&count_sql, params![project_id], |row| row.get(0))?
        };

        // Get assets
        let sql = format!(
            r#"
            SELECT a.id, a.project_id, a.absolute_path, a.relative_path, a.file_name,
                   a.extension, a.asset_type, a.size_bytes, a.modified_time, a.content_hash,
                   a.unity_guid, a.import_type, a.thumbnail_path, a.created_at, a.updated_at
            FROM assets a
            {}{}
            ORDER BY a.file_name ASC
            LIMIT ?2 OFFSET ?3
            "#,
            where_clause, type_filter
        );

        let mut stmt = conn.prepare(&sql)?;

        let assets: Vec<Asset> = if let Some(query) = search_query {
            if query.trim().is_empty() {
                stmt.query_map(params![project_id, page_size, offset], |row| {
                    Ok(Asset {
                        id: row.get(0)?,
                        project_id: row.get(1)?,
                        absolute_path: row.get(2)?,
                        relative_path: row.get(3)?,
                        file_name: row.get(4)?,
                        extension: row.get(5)?,
                        asset_type: row.get(6)?,
                        size_bytes: row.get(7)?,
                        modified_time: row.get(8)?,
                        content_hash: row.get(9)?,
                        unity_guid: row.get(10)?,
                        import_type: row.get(11)?,
                        thumbnail_path: row.get(12)?,
                        created_at: row.get(13)?,
                        updated_at: row.get(14)?,
                    })
                })?
                .filter_map(|r| r.ok())
                .collect()
            } else {
                let fts_query = format!("{}*", query);
                stmt.query_map(params![project_id, page_size, offset, fts_query], |row| {
                    Ok(Asset {
                        id: row.get(0)?,
                        project_id: row.get(1)?,
                        absolute_path: row.get(2)?,
                        relative_path: row.get(3)?,
                        file_name: row.get(4)?,
                        extension: row.get(5)?,
                        asset_type: row.get(6)?,
                        size_bytes: row.get(7)?,
                        modified_time: row.get(8)?,
                        content_hash: row.get(9)?,
                        unity_guid: row.get(10)?,
                        import_type: row.get(11)?,
                        thumbnail_path: row.get(12)?,
                        created_at: row.get(13)?,
                        updated_at: row.get(14)?,
                    })
                })?
                .filter_map(|r| r.ok())
                .collect()
            }
        } else {
            stmt.query_map(params![project_id, page_size, offset], |row| {
                Ok(Asset {
                    id: row.get(0)?,
                    project_id: row.get(1)?,
                    absolute_path: row.get(2)?,
                    relative_path: row.get(3)?,
                    file_name: row.get(4)?,
                    extension: row.get(5)?,
                    asset_type: row.get(6)?,
                    size_bytes: row.get(7)?,
                    modified_time: row.get(8)?,
                    content_hash: row.get(9)?,
                    unity_guid: row.get(10)?,
                    import_type: row.get(11)?,
                    thumbnail_path: row.get(12)?,
                    created_at: row.get(13)?,
                    updated_at: row.get(14)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect()
        };

        Ok((assets, total))
    }

    pub fn get_asset(&self, id: &str) -> AppResult<Option<Asset>> {
        let conn = self.pool.get()?;

        let result = conn
            .query_row(
                r#"
                SELECT id, project_id, absolute_path, relative_path, file_name, extension,
                       asset_type, size_bytes, modified_time, content_hash, unity_guid,
                       import_type, thumbnail_path, created_at, updated_at
                FROM assets WHERE id = ?1
                "#,
                params![id],
                |row| {
                    Ok(Asset {
                        id: row.get(0)?,
                        project_id: row.get(1)?,
                        absolute_path: row.get(2)?,
                        relative_path: row.get(3)?,
                        file_name: row.get(4)?,
                        extension: row.get(5)?,
                        asset_type: row.get(6)?,
                        size_bytes: row.get(7)?,
                        modified_time: row.get(8)?,
                        content_hash: row.get(9)?,
                        unity_guid: row.get(10)?,
                        import_type: row.get(11)?,
                        thumbnail_path: row.get(12)?,
                        created_at: row.get(13)?,
                        updated_at: row.get(14)?,
                    })
                },
            )
            .ok();

        Ok(result)
    }

    pub fn get_asset_by_guid(&self, project_id: &str, guid: &str) -> AppResult<Option<Asset>> {
        let conn = self.pool.get()?;

        let result = conn
            .query_row(
                r#"
                SELECT id, project_id, absolute_path, relative_path, file_name, extension,
                       asset_type, size_bytes, modified_time, content_hash, unity_guid,
                       import_type, thumbnail_path, created_at, updated_at
                FROM assets WHERE project_id = ?1 AND unity_guid = ?2
                "#,
                params![project_id, guid],
                |row| {
                    Ok(Asset {
                        id: row.get(0)?,
                        project_id: row.get(1)?,
                        absolute_path: row.get(2)?,
                        relative_path: row.get(3)?,
                        file_name: row.get(4)?,
                        extension: row.get(5)?,
                        asset_type: row.get(6)?,
                        size_bytes: row.get(7)?,
                        modified_time: row.get(8)?,
                        content_hash: row.get(9)?,
                        unity_guid: row.get(10)?,
                        import_type: row.get(11)?,
                        thumbnail_path: row.get(12)?,
                        created_at: row.get(13)?,
                        updated_at: row.get(14)?,
                    })
                },
            )
            .ok();

        Ok(result)
    }

    pub fn get_type_counts(&self, project_id: &str) -> AppResult<Vec<TypeCount>> {
        let conn = self.pool.get()?;

        let mut stmt = conn.prepare(
            "SELECT asset_type, COUNT(*) as count FROM assets WHERE project_id = ?1 GROUP BY asset_type",
        )?;

        let counts: Vec<TypeCount> = stmt
            .query_map(params![project_id], |row| {
                Ok(TypeCount {
                    asset_type: row.get(0)?,
                    count: row.get(1)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(counts)
    }

    pub fn insert_dependency(&self, dep: &Dependency) -> AppResult<()> {
        let conn = self.pool.get()?;

        conn.execute(
            r#"
            INSERT OR REPLACE INTO dependencies (id, from_asset_id, to_asset_id, to_guid, relation_type, confidence, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
            params![
                dep.id,
                dep.from_asset_id,
                dep.to_asset_id,
                dep.to_guid,
                dep.relation_type,
                dep.confidence,
                dep.created_at,
            ],
        )?;

        Ok(())
    }

    pub fn get_dependencies(&self, asset_id: &str) -> AppResult<Vec<Dependency>> {
        let conn = self.pool.get()?;

        let mut stmt = conn.prepare(
            "SELECT id, from_asset_id, to_asset_id, to_guid, relation_type, confidence, created_at
             FROM dependencies WHERE from_asset_id = ?1",
        )?;

        let deps: Vec<Dependency> = stmt
            .query_map(params![asset_id], |row| {
                Ok(Dependency {
                    id: row.get(0)?,
                    from_asset_id: row.get(1)?,
                    to_asset_id: row.get(2)?,
                    to_guid: row.get(3)?,
                    relation_type: row.get(4)?,
                    confidence: row.get(5)?,
                    created_at: row.get(6)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(deps)
    }

    pub fn get_dependents(&self, asset_id: &str) -> AppResult<Vec<Dependency>> {
        let conn = self.pool.get()?;

        let mut stmt = conn.prepare(
            "SELECT id, from_asset_id, to_asset_id, to_guid, relation_type, confidence, created_at
             FROM dependencies WHERE to_asset_id = ?1",
        )?;

        let deps: Vec<Dependency> = stmt
            .query_map(params![asset_id], |row| {
                Ok(Dependency {
                    id: row.get(0)?,
                    from_asset_id: row.get(1)?,
                    to_asset_id: row.get(2)?,
                    to_guid: row.get(3)?,
                    relation_type: row.get(4)?,
                    confidence: row.get(5)?,
                    created_at: row.get(6)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(deps)
    }

    pub fn delete_dependencies_for_asset(&self, asset_id: &str) -> AppResult<()> {
        let conn = self.pool.get()?;
        conn.execute("DELETE FROM dependencies WHERE from_asset_id = ?1", params![asset_id])?;
        Ok(())
    }

    pub fn update_asset_thumbnail(&self, asset_id: &str, thumbnail_path: &str) -> AppResult<()> {
        let conn = self.pool.get()?;
        conn.execute(
            "UPDATE assets SET thumbnail_path = ?1 WHERE id = ?2",
            params![thumbnail_path, asset_id],
        )?;
        Ok(())
    }

    pub fn get_assets_needing_thumbnails(&self, project_id: &str, limit: i64) -> AppResult<Vec<Asset>> {
        let conn = self.pool.get()?;

        let mut stmt = conn.prepare(
            r#"
            SELECT id, project_id, absolute_path, relative_path, file_name, extension,
                   asset_type, size_bytes, modified_time, content_hash, unity_guid,
                   import_type, thumbnail_path, created_at, updated_at
            FROM assets
            WHERE project_id = ?1
              AND asset_type IN ('texture', 'material')
              AND thumbnail_path IS NULL
            ORDER BY
              CASE asset_type
                WHEN 'texture' THEN 1
                WHEN 'material' THEN 2
                ELSE 3
              END
            LIMIT ?2
            "#,
        )?;

        let assets: Vec<Asset> = stmt
            .query_map(params![project_id, limit], |row| {
                Ok(Asset {
                    id: row.get(0)?,
                    project_id: row.get(1)?,
                    absolute_path: row.get(2)?,
                    relative_path: row.get(3)?,
                    file_name: row.get(4)?,
                    extension: row.get(5)?,
                    asset_type: row.get(6)?,
                    size_bytes: row.get(7)?,
                    modified_time: row.get(8)?,
                    content_hash: row.get(9)?,
                    unity_guid: row.get(10)?,
                    import_type: row.get(11)?,
                    thumbnail_path: row.get(12)?,
                    created_at: row.get(13)?,
                    updated_at: row.get(14)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(assets)
    }

    pub fn get_parseable_assets(&self, project_id: &str) -> AppResult<Vec<Asset>> {
        let conn = self.pool.get()?;

        let mut stmt = conn.prepare(
            r#"
            SELECT id, project_id, absolute_path, relative_path, file_name, extension,
                   asset_type, size_bytes, modified_time, content_hash, unity_guid,
                   import_type, thumbnail_path, created_at, updated_at
            FROM assets
            WHERE project_id = ?1
              AND asset_type IN ('material', 'prefab', 'scene', 'scriptable_object')
            "#,
        )?;

        let assets: Vec<Asset> = stmt
            .query_map(params![project_id], |row| {
                Ok(Asset {
                    id: row.get(0)?,
                    project_id: row.get(1)?,
                    absolute_path: row.get(2)?,
                    relative_path: row.get(3)?,
                    file_name: row.get(4)?,
                    extension: row.get(5)?,
                    asset_type: row.get(6)?,
                    size_bytes: row.get(7)?,
                    modified_time: row.get(8)?,
                    content_hash: row.get(9)?,
                    unity_guid: row.get(10)?,
                    import_type: row.get(11)?,
                    thumbnail_path: row.get(12)?,
                    created_at: row.get(13)?,
                    updated_at: row.get(14)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(assets)
    }
}
