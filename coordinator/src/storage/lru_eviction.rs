use std::path::PathBuf;
use std::time::Duration;
use tracing::{error, info};

pub struct LruEviction {
    db: sqlx::PgPool,
    wasm_cache_dir: PathBuf,
    max_size_bytes: u64,
}

impl LruEviction {
    pub fn new(db: sqlx::PgPool, wasm_cache_dir: PathBuf, max_size_bytes: u64) -> Self {
        Self {
            db,
            wasm_cache_dir,
            max_size_bytes,
        }
    }

    pub async fn check_and_evict(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Calculate current cache size
        let current_size: i64 = sqlx::query_scalar!(
            "SELECT COALESCE(SUM(file_size), 0)::bigint FROM wasm_cache"
        )
        .fetch_one(&self.db)
        .await?
        .unwrap_or(0);

        let current_size = current_size as u64;

        if current_size <= self.max_size_bytes {
            return Ok(());
        }

        info!(
            "Cache size {} bytes exceeds limit {} bytes, starting eviction",
            current_size, self.max_size_bytes
        );

        // Need to evict - get LRU items
        let bytes_to_free = current_size - self.max_size_bytes;
        let mut bytes_freed = 0u64;

        let lru_items = sqlx::query!(
            "SELECT checksum, file_size FROM wasm_cache ORDER BY last_accessed_at ASC"
        )
        .fetch_all(&self.db)
        .await?;

        for item in lru_items {
            if bytes_freed >= bytes_to_free {
                break;
            }

            // Delete from filesystem
            let wasm_path = self.wasm_cache_dir.join(format!("{}.wasm", item.checksum));
            if let Err(e) = tokio::fs::remove_file(&wasm_path).await {
                error!("Failed to delete WASM file {}: {}", item.checksum, e);
            }

            // Delete from database
            sqlx::query!("DELETE FROM wasm_cache WHERE checksum = $1", item.checksum)
                .execute(&self.db)
                .await?;

            bytes_freed += item.file_size as u64;
            info!("Evicted WASM: {} ({} bytes)", item.checksum, item.file_size);
        }

        info!("LRU eviction freed {} bytes", bytes_freed);
        Ok(())
    }

    pub async fn run_periodic_check(&self, interval: Duration) {
        loop {
            tokio::time::sleep(interval).await;
            if let Err(e) = self.check_and_evict().await {
                error!("LRU eviction error: {}", e);
            }
        }
    }
}
