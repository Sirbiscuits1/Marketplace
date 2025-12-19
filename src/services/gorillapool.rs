use crate::config::Config;
use crate::models::{Inscription, OrdinalUtxo};
use anyhow::{Context, Result};
use governor::{Quota, RateLimiter};
use reqwest::Client;
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;
use tracing::{debug, error, info};

/// GorillaPool API client with built-in rate limiting
pub struct GorillaPoolClient {
    client: Client,
    base_url: String,
    rate_limiter: Arc<RateLimiter<governor::state::NotKeyed, governor::state::InMemoryState, governor::clock::DefaultClock>>,
    concurrent_semaphore: Arc<Semaphore>,
}

impl GorillaPoolClient {
    pub fn new(config: &Config) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .pool_max_idle_per_host(10)
            .build()
            .context("Failed to create HTTP client")?;

        let quota = Quota::per_second(NonZeroU32::new(config.api_rate_limit_per_second).unwrap())
            .allow_burst(NonZeroU32::new(config.api_rate_limit_burst).unwrap());
        
        let rate_limiter = Arc::new(RateLimiter::direct(quota));
        let concurrent_semaphore = Arc::new(Semaphore::new(config.max_concurrent_requests));

        info!(
            "GorillaPool client initialized: {} req/sec, burst: {}, concurrent: {}",
            config.api_rate_limit_per_second,
            config.api_rate_limit_burst,
            config.max_concurrent_requests
        );

        Ok(Self {
            client,
            base_url: config.gorillapool_base_url.clone(),
            rate_limiter,
            concurrent_semaphore,
        })
    }

    async fn wait_for_rate_limit(&self) {
        self.rate_limiter.until_ready().await;
    }

    /// Get all ordinal UTXOs for an address using the CORRECT endpoint
    /// Endpoint: GET /api/txos/address/:address/unspent
    pub async fn get_address_utxos(&self, address: &str) -> Result<Vec<OrdinalUtxo>> {
        let _permit = self.concurrent_semaphore.acquire().await?;
        self.wait_for_rate_limit().await;

        // Use the correct endpoint: /txos/address/:address/unspent
        let url = format!("{}/txos/address/{}/unspent", self.base_url, address);
        debug!("Fetching UTXOs from: {}", url);

        let response = self.client.get(&url).send().await.context("Failed to fetch UTXOs")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            
            if status.as_u16() == 404 {
                debug!("No UTXOs found for address: {}", address);
                return Ok(vec![]);
            }
            
            error!("GorillaPool API error: {} - {}", status, body);
            anyhow::bail!("GorillaPool API returned {}: {}", status, body);
        }

        let utxos: Vec<OrdinalUtxo> = response.json().await.context("Failed to parse UTXO response")?;
        debug!("Found {} UTXOs for address {}", utxos.len(), address);
        Ok(utxos)
    }

    /// Get UTXOs with full inscription data - uses txos endpoint
    pub async fn get_address_inscriptions(&self, address: &str) -> Result<Vec<serde_json::Value>> {
        let _permit = self.concurrent_semaphore.acquire().await?;
        self.wait_for_rate_limit().await;

        // Use the correct endpoint that actually works
        let url = format!("{}/txos/address/{}/unspent", self.base_url, address);
        debug!("Fetching inscriptions from: {}", url);

        let response = self.client.get(&url).send().await.context("Failed to fetch inscriptions")?;

        if !response.status().is_success() {
            let status = response.status();
            if status.as_u16() == 404 {
                debug!("No inscriptions found for address: {}", address);
                return Ok(vec![]);
            }
            
            let body = response.text().await.unwrap_or_default();
            error!("GorillaPool API error: {} - {}", status, body);
            anyhow::bail!("GorillaPool API returned {}: {}", status, body);
        }

        // Return raw JSON since the structure is different from what we expected
        let inscriptions: Vec<serde_json::Value> = response.json().await.context("Failed to parse inscriptions response")?;
        debug!("Found {} inscriptions for address {}", inscriptions.len(), address);
        Ok(inscriptions)
    }

    /// Get inscription details by origin
    pub async fn get_inscription_by_origin(&self, origin: &str) -> Result<Option<Inscription>> {
        let _permit = self.concurrent_semaphore.acquire().await?;
        self.wait_for_rate_limit().await;

        let url = format!("{}/inscriptions/origin/{}", self.base_url, origin);
        debug!("Fetching inscription: {}", url);

        let response = self.client.get(&url).send().await.context("Failed to fetch inscription")?;

        if response.status().as_u16() == 404 {
            return Ok(None);
        }

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            error!("GorillaPool API error: {} - {}", status, body);
            anyhow::bail!("GorillaPool API returned {}: {}", status, body);
        }

        let inscription: Inscription = response.json().await.context("Failed to parse inscription response")?;
        Ok(Some(inscription))
    }

    /// Get inscription content
    pub async fn get_inscription_content(&self, origin: &str) -> Result<(Vec<u8>, String)> {
        let _permit = self.concurrent_semaphore.acquire().await?;
        self.wait_for_rate_limit().await;

        let url = format!("{}/files/inscriptions/{}", self.base_url, origin);
        debug!("Fetching content: {}", url);

        let response = self.client.get(&url).send().await.context("Failed to fetch inscription content")?;

        if !response.status().is_success() {
            let status = response.status();
            anyhow::bail!("Failed to fetch content: {}", status);
        }

        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("application/octet-stream")
            .to_string();

        let bytes = response.bytes().await?.to_vec();
        debug!("Fetched {} bytes of content type: {}", bytes.len(), content_type);
        Ok((bytes, content_type))
    }

    pub fn content_url(&self, origin: &str) -> String {
        format!("{}/files/inscriptions/{}", self.base_url, origin)
    }

    pub fn preview_url(&self, origin: &str) -> String {
        self.content_url(origin)
    }
}

impl Clone for GorillaPoolClient {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            base_url: self.base_url.clone(),
            rate_limiter: Arc::clone(&self.rate_limiter),
            concurrent_semaphore: Arc::clone(&self.concurrent_semaphore),
        }
    }
}
