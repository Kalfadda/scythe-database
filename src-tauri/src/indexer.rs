use crate::db::{Asset, Database};
use crate::error::AppResult;
use std::sync::Arc;

pub struct Indexer {
    db: Arc<Database>,
}

impl Indexer {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    pub fn upsert_batch(&self, assets: &[Asset]) -> AppResult<usize> {
        let conn = self.db.pool().get()?;

        // Use a transaction for batch insert
        conn.execute("BEGIN TRANSACTION", [])?;

        let mut count = 0;
        for asset in assets {
            if let Err(e) = self.db.upsert_asset(asset) {
                tracing::warn!("Failed to upsert asset {}: {}", asset.relative_path, e);
                continue;
            }
            count += 1;
        }

        conn.execute("COMMIT", [])?;

        Ok(count)
    }
}
