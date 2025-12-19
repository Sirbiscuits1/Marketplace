use crate::config::Config;
use crate::models::{CacheStats, OrdinalDetails, WalletOrdinals};
use moka::future::Cache;
use std::sync::atomic::{AtomicU64, Ordering};
use tracing::{debug, info};

/// Cache manager for ordinal data with different TTLs per data type
pub struct CacheManager {
    wallet_cache: Cache<String, WalletOrdinals>,
    ordinal_cache: Cache<String, OrdinalDetails>,
    content_cache: Cache<String, (Vec<u8>, String)>,
    hits: AtomicU64,
    misses: AtomicU64,
}

impl CacheManager {
    pub fn new(config: &Config) -> Self {
        let wallet_cache = Cache::builder()
            .max_capacity(config.max_cache_entries)
            .time_to_live(config.ownership_cache_ttl)
            .build();

        let ordinal_cache = Cache::builder()
            .max_capacity(config.max_cache_entries)
            .time_to_live(config.metadata_cache_ttl)
            .build();

        let content_cache = Cache::builder()
            .max_capacity(config.max_cache_entries / 10)
            .time_to_live(config.content_cache_ttl)
            .build();

        info!(
            "Cache initialized: wallet TTL={}s, metadata TTL={}s, content TTL={}s",
            config.ownership_cache_ttl.as_secs(),
            config.metadata_cache_ttl.as_secs(),
            config.content_cache_ttl.as_secs()
        );

        Self {
            wallet_cache,
            ordinal_cache,
            content_cache,
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
        }
    }

    pub async fn get_wallet_ordinals(&self, address: &str) -> Option<WalletOrdinals> {
        let key = format!("wallet:{}", address);
        match self.wallet_cache.get(&key).await {
            Some(v) => {
                self.hits.fetch_add(1, Ordering::Relaxed);
                debug!("Cache HIT: {}", key);
                Some(v)
            }
            None => {
                self.misses.fetch_add(1, Ordering::Relaxed);
                debug!("Cache MISS: {}", key);
                None
            }
        }
    }

    pub async fn set_wallet_ordinals(&self, address: &str, data: &WalletOrdinals) {
        let key = format!("wallet:{}", address);
        self.wallet_cache.insert(key, data.clone()).await;
    }

    pub async fn invalidate_wallet(&self, address: &str) {
        let key = format!("wallet:{}", address);
        self.wallet_cache.invalidate(&key).await;
        debug!("Invalidated wallet cache: {}", address);
    }

    pub async fn get_ordinal_details(&self, origin: &str) -> Option<OrdinalDetails> {
        let key = format!("ordinal:{}", origin);
        match self.ordinal_cache.get(&key).await {
            Some(v) => {
                self.hits.fetch_add(1, Ordering::Relaxed);
                Some(v)
            }
            None => {
                self.misses.fetch_add(1, Ordering::Relaxed);
                None
            }
        }
    }

    pub async fn set_ordinal_details(&self, origin: &str, data: &OrdinalDetails) {
        let key = format!("ordinal:{}", origin);
        self.ordinal_cache.insert(key, data.clone()).await;
    }

    pub async fn get_content(&self, origin: &str) -> Option<(Vec<u8>, String)> {
        let key = format!("content:{}", origin);
        match self.content_cache.get(&key).await {
            Some(v) => {
                self.hits.fetch_add(1, Ordering::Relaxed);
                Some(v)
            }
            None => {
                self.misses.fetch_add(1, Ordering::Relaxed);
                None
            }
        }
    }

    pub async fn set_content(&self, origin: &str, data: &[u8], content_type: &str) {
        let key = format!("content:{}", origin);
        self.content_cache.insert(key, (data.to_vec(), content_type.to_string())).await;
    }

    pub fn stats(&self) -> CacheStats {
        let hits = self.hits.load(Ordering::Relaxed);
        let misses = self.misses.load(Ordering::Relaxed);
        let total = hits + misses;
        
        let hit_rate = if total > 0 {
            (hits as f64 / total as f64) * 100.0
        } else {
            0.0
        };

        CacheStats {
            ownership_entries: self.wallet_cache.entry_count(),
            content_entries: self.content_cache.entry_count(),
            hit_rate_percent: hit_rate,
        }
    }

    pub async fn clear_all(&self) {
        self.wallet_cache.invalidate_all();
        self.ordinal_cache.invalidate_all();
        self.content_cache.invalidate_all();
        info!("All caches cleared");
    }
}
