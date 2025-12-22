mod api;
mod cache;
mod config;
mod models;
mod services;

use api::create_router;
use api::handlers::AppState;  // ← Import the correct AppState from handlers.rs
use cache::CacheManager;
use config::Config;
use services::{GorillaPoolClient, OrdinalService, ListingsDb};
use std::sync::Arc;
use std::time::Instant;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    let _subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .with_target(true)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false)
        .init();

    info!("🚀 BSV 1Sat Ordinals Marketplace starting...");

    // Load configuration
    let config = Config::from_env();
    info!("Configuration loaded: {}:{}", config.server_addr, config.server_port);

    // Initialize database
    let db = sled::open(&config.db_path)?;
    let db = Arc::new(db);
    info!("Database opened at: {}", config.db_path);

    // Initialize services
    let gorillapool = GorillaPoolClient::new(&config)
        .expect("Failed to create GorillaPool client");
    
    let cache = Arc::new(CacheManager::new(&config));
    
    let ordinal_service = OrdinalService::new(
        gorillapool,
        Arc::clone(&cache),
        config.clone(),
    );

    let listings_db = ListingsDb::new(Arc::clone(&db));
    let active_listings = listings_db.count_active_listings();
    info!("Listings database loaded: {} active listings", active_listings);

    // Create application state — using the AppState from handlers.rs
    let state = AppState {
        ordinal_service,
        cache,
        listings_db,
        start_time: Instant::now(),
        config: config.clone(),
    };

    // Build router
    let app = create_router(state);

    // Bind and serve
    let addr = format!("{}:{}", config.server_addr, config.server_port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    
    info!("✅ Server running at http://{}", addr);
    info!("📖 API Endpoints:");
    info!("   GET  /                        → API info");
    info!("   GET  /health                  → Health check");
    info!("   GET  /wallet/:address         → Get wallet ordinals");
    info!("   GET  /ordinal/:origin         → Get ordinal details");
    info!("   GET  /ordinal/:origin/content → Get content");
    info!("   GET  /listings                → Get active listings");
    info!("   POST /listings                → Create listing");
    info!("   POST /listings/:id/cancel     → Cancel listing");
    info!("   POST /listings/:id/prepare-purchase → Prepare unsigned TX for Yours Wallet purchase");
    info!("   POST /listings/:id/purchase   → Purchase listing");
    info!("   GET  /fees/calculate          → Calculate fees");
    info!("");

    axum::serve(listener, app).await?;

    Ok(())
}