mod handlers;

pub use handlers::{
    AppState, root, health, 
    get_wallet_ordinals, get_ordinal_details, get_ordinal_content, 
    search_ordinals,
    get_listings, get_listing, create_listing, cancel_listing, purchase_listing,
    get_listing_by_origin, calculate_fees,
};

use axum::{routing::{get, post}, Router};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

/// Build the API router with all routes
pub fn create_router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        // Info endpoints
        .route("/", get(root))
        .route("/health", get(health))
        
        // Wallet endpoints
        .route("/wallet/:address", get(get_wallet_ordinals))
        
        // Ordinal endpoints
        .route("/ordinal/:origin", get(get_ordinal_details))
        .route("/ordinal/:origin/content", get(get_ordinal_content))
        .route("/ordinal/:origin/listing", get(get_listing_by_origin))
        
        // Listings endpoints
        .route("/listings", get(get_listings))
        .route("/listings", post(create_listing))
        .route("/listings/:id", get(get_listing))
        .route("/listings/:id/cancel", post(cancel_listing))
        .route("/listings/:id/purchase", post(purchase_listing))
        
        // Fee calculation
        .route("/fees/calculate", get(calculate_fees))
        
        // Search
        .route("/search", get(search_ordinals))
        
        // Middleware
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        
        // State
        .with_state(state)
}
