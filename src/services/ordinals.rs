use crate::cache::CacheManager;
use crate::config::Config;
use crate::models::{OrdinalDetails, WalletOrdinals};
use crate::services::GorillaPoolClient;
use anyhow::{Context, Result};
use chrono::Utc;
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, info, warn};

/// Main ordinals service - coordinates fetching, caching, and enrichment
pub struct OrdinalService {
    gorillapool: GorillaPoolClient,
    cache: Arc<CacheManager>,
    config: Config,
}

impl OrdinalService {
    pub fn new(gorillapool: GorillaPoolClient, cache: Arc<CacheManager>, config: Config) -> Self {
        Self { gorillapool, cache, config }
    }

    /// Get all ordinals for a wallet address
    pub async fn get_wallet_ordinals(&self, address: &str) -> Result<WalletOrdinals> {
        let start = Instant::now();
        info!("Fetching ordinals for address: {}", address);

        if let Some(cached) = self.cache.get_wallet_ordinals(address).await {
            debug!("Cache hit for wallet: {}", address);
            return Ok(cached);
        }

        // Fetch from GorillaPool using the correct endpoint
        let raw_inscriptions = self.gorillapool
            .get_address_inscriptions(address)
            .await
            .context("Failed to fetch inscriptions for address")?;

        debug!("Found {} raw items for {}", raw_inscriptions.len(), address);

        // Parse the response into our format
        let mut ordinals: Vec<OrdinalDetails> = Vec::new();

        for item in raw_inscriptions {
            // Only process items that have origin data (actual inscriptions)
            if let Some(origin_data) = item.get("origin") {
                if origin_data.is_null() {
                    continue; // Skip non-inscription UTXOs
                }

                let outpoint = item.get("outpoint")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                
                let txid = item.get("txid")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                
                let vout = item.get("vout")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32;

                let satoshis = item.get("satoshis")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(1);

                let height = item.get("height")
                    .and_then(|v| v.as_u64());

                // Extract file info from origin.data.insc.file
                let (content_type, content_size, content_hash) = if let Some(data) = origin_data.get("data") {
                    if let Some(insc) = data.get("insc") {
                        if let Some(file) = insc.get("file") {
                            let ct = file.get("type").and_then(|v| v.as_str()).map(|s| s.to_string());
                            let size = file.get("size").and_then(|v| v.as_u64());
                            let hash = file.get("hash").and_then(|v| v.as_str()).map(|s| s.to_string());
                            (ct, size, hash)
                        } else {
                            (None, None, None)
                        }
                    } else {
                        (None, None, None)
                    }
                } else {
                    (None, None, None)
                };

                // Extract metadata (MAP data)
                let metadata = origin_data.get("data")
                    .and_then(|d| d.get("map"))
                    .cloned();

                // Extract collection ID if present
                let collection_id = metadata.as_ref()
                    .and_then(|m| m.get("subTypeData"))
                    .and_then(|s| s.get("collectionId"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                // Get the origin outpoint for content URL
                let origin_outpoint = origin_data.get("outpoint")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&outpoint)
                    .to_string();

                // Extract inscription number from origin.num (format: "0927773:116:0")
                let inscription_number = origin_data.get("num")
                    .and_then(|v| v.as_str())
                    .and_then(|s| {
                        // Parse the first number from "0927773:116:0" format
                        s.split(':').next()
                            .and_then(|n| n.parse::<u64>().ok())
                    });

                let details = OrdinalDetails {
                    origin: origin_outpoint.clone(),
                    txid,
                    vout,
                    owner_address: address.to_string(),
                    satoshis,
                    content_type,
                    content_size,
                    content_hash,
                    block_height: height,
                    inscription_number,
                    metadata,
                    collection_id,
                    content_url: self.gorillapool.content_url(&origin_outpoint),
                    preview_url: self.gorillapool.preview_url(&origin_outpoint),
                    fetched_at: Utc::now(),
                };

                self.cache.set_ordinal_details(&origin_outpoint, &details).await;
                ordinals.push(details);
            }
        }

        let fetch_time_ms = start.elapsed().as_millis() as u64;
        
        let wallet_data = WalletOrdinals {
            address: address.to_string(),
            total_count: ordinals.len(),
            ordinals,
            fetched_at: Utc::now(),
            fetch_time_ms,
        };

        self.cache.set_wallet_ordinals(address, &wallet_data).await;

        info!(
            "Fetched {} ordinals for {} in {}ms",
            wallet_data.total_count, address, fetch_time_ms
        );

        Ok(wallet_data)
    }

    /// Get details for a specific ordinal by origin
    pub async fn get_ordinal_details(&self, origin: &str) -> Result<Option<OrdinalDetails>> {
        if let Some(cached) = self.cache.get_ordinal_details(origin).await {
            debug!("Cache hit for ordinal: {}", origin);
            return Ok(Some(cached));
        }

        // For now, return None if not in cache
        // Full implementation would query by origin
        warn!("Ordinal not in cache: {}", origin);
        Ok(None)
    }

    /// Get inscription content
    pub async fn get_ordinal_content(&self, origin: &str) -> Result<(Vec<u8>, String)> {
        if let Some(cached) = self.cache.get_content(origin).await {
            debug!("Cache hit for content: {}", origin);
            return Ok(cached);
        }

        let (content, content_type) = self.gorillapool
            .get_inscription_content(origin)
            .await
            .context("Failed to fetch inscription content")?;

        self.cache.set_content(origin, &content, &content_type).await;
        Ok((content, content_type))
    }

    /// Force refresh a wallet's ordinals
    pub async fn refresh_wallet(&self, address: &str) -> Result<WalletOrdinals> {
        self.cache.invalidate_wallet(address).await;
        self.get_wallet_ordinals(address).await
    }

    pub fn gorillapool(&self) -> &GorillaPoolClient {
        &self.gorillapool
    }
}

impl Clone for OrdinalService {
    fn clone(&self) -> Self {
        Self {
            gorillapool: self.gorillapool.clone(),
            cache: Arc::clone(&self.cache),
            config: self.config.clone(),
        }
    }
}
