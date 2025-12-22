use std::time::Duration;

/// Application configuration
#[derive(Clone, Debug)]
pub struct Config {
    /// Server bind address
    pub server_addr: String,
    /// Server port
    pub server_port: u16,
    
    /// GorillaPool API base URL
    pub gorillapool_base_url: String,
    /// WhatsOnChain API base URL  
    pub whatsonchain_base_url: String,
    
    /// Rate limit: max requests per second to external APIs
    pub api_rate_limit_per_second: u32,
    /// Rate limit: burst capacity
    pub api_rate_limit_burst: u32,
    
    /// Cache TTL for ownership data (shorter - ownership changes)
    pub ownership_cache_ttl: Duration,
    /// Cache TTL for inscription content (longer - content is immutable)
    pub content_cache_ttl: Duration,
    /// Cache TTL for inscription metadata
    pub metadata_cache_ttl: Duration,
    /// Maximum cache entries
    pub max_cache_entries: u64,
    
    /// Concurrent API request limit
    pub max_concurrent_requests: usize,
    
    /// Database path
    pub db_path: String,

    /// BSV address that receives the 1% marketplace fee and optional tips
    pub marketplace_fee_address: String,

    /// HandCash App ID (public)
    pub handcash_app_id: String,
    /// HandCash App Secret (server-only - keep secret!)
    pub handcash_app_secret: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server_addr: "0.0.0.0".to_string(),
            server_port: 3000,
            
            gorillapool_base_url: "https://ordinals.gorillapool.io/api".to_string(),
            whatsonchain_base_url: "https://plugins.whatsonchain.com/api/plugin/main".to_string(),
            
            // Conservative rate limiting to stay well under ceiling
            api_rate_limit_per_second: 10,
            api_rate_limit_burst: 20,
            
            // Cache durations
            ownership_cache_ttl: Duration::from_secs(30),
            content_cache_ttl: Duration::from_secs(86400),
            metadata_cache_ttl: Duration::from_secs(300),
            max_cache_entries: 10_000,
            
            max_concurrent_requests: 5,
            
            db_path: "marketplace_db".to_string(),

            // Real marketplace fee address
            marketplace_fee_address: "15BvxtG9U61ndVZccSmuG9nQzygzjDqC41".to_string(),

            // Real HandCash App ID (public - safe to hardcode)
            handcash_app_id: "68082b6ba117aae3817ec15f".to_string(),

            // Placeholder for secret - MUST be overridden in production via env var
            handcash_app_secret: "PLACEHOLDER_SECRET_DO_NOT_USE_IN_PRODUCTION".to_string(),
        }
    }
}

impl Config {
    /// Load config from environment variables with defaults
    pub fn from_env() -> Self {
        let mut config = Self::default();
        
        if let Ok(port) = std::env::var("PORT") {
            if let Ok(p) = port.parse() {
                config.server_port = p;
            }
        }
        
        if let Ok(path) = std::env::var("DB_PATH") {
            config.db_path = path;
        }
        
        if let Ok(rate) = std::env::var("API_RATE_LIMIT") {
            if let Ok(r) = rate.parse() {
                config.api_rate_limit_per_second = r;
            }
        }

        // Load marketplace fee address - REQUIRED in production
        if let Ok(addr) = std::env::var("MARKETPLACE_FEE_ADDRESS") {
            config.marketplace_fee_address = addr;
        } else if cfg!(debug_assertions) {
            tracing::warn!("MARKETPLACE_FEE_ADDRESS not set - using default");
        } else {
            panic!("MARKETPLACE_FEE_ADDRESS environment variable is required");
        }

        // HandCash App ID - public, can fall back to default
        config.handcash_app_id = std::env::var("HANDCASH_APP_ID")
            .unwrap_or_else(|_| config.handcash_app_id.clone());

        // HandCash App Secret - REQUIRED in production
        config.handcash_app_secret = std::env::var("HANDCASH_APP_SECRET")
            .unwrap_or_else(|_| {
                if cfg!(debug_assertions) {
                    tracing::warn!("HANDCASH_APP_SECRET not set - HandCash features will fail");
                    config.handcash_app_secret.clone()
                } else {
                    panic!("HANDCASH_APP_SECRET is required for HandCash integration");
                }
            });

        config
    }
}