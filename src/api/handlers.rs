use crate::cache::CacheManager;
use crate::models::{
    ApiError, HealthCheck, CreateListingRequest, CreateListingResponse,
    CancelListingRequest, PurchaseListingRequest, ListingsResponse, ListingsQuery,
    ListingFees,
};
use crate::services::OrdinalService;
use crate::services::ListingsDb;
use axum::{
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Instant;
use tracing::{error, info};

/// Application state shared across handlers
#[derive(Clone)]
pub struct AppState {
    pub ordinal_service: OrdinalService,
    pub cache: Arc<CacheManager>,
    pub listings_db: ListingsDb,
    pub start_time: Instant,
}

// ============================================================================
// Query parameters
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct RefreshParam {
    #[serde(default)]
    pub refresh: bool,
}

// ============================================================================
// Response types
// ============================================================================

#[derive(Serialize)]
pub struct WalletResponse {
    pub success: bool,
    pub data: crate::models::WalletOrdinals,
}

#[derive(Serialize)]
pub struct OrdinalResponse {
    pub success: bool,
    pub data: crate::models::OrdinalDetails,
}

#[derive(Serialize)]
pub struct FeeCalculationResponse {
    pub success: bool,
    pub fees: ListingFees,
}

// ============================================================================
// Info Handlers
// ============================================================================

/// Root endpoint - API info
pub async fn root() -> impl IntoResponse {
    let info = serde_json::json!({
        "name": "BSV 1Sat Ordinals Marketplace API",
        "version": env!("CARGO_PKG_VERSION"),
        "endpoints": {
            "GET /": "This help message",
            "GET /health": "Health check and cache stats",
            "GET /wallet/:address": "Get all ordinals for a wallet address",
            "GET /ordinal/:origin": "Get details for a specific ordinal",
            "GET /ordinal/:origin/content": "Get ordinal content (image/file)",
            "GET /listings": "Get active marketplace listings",
            "GET /listings/:id": "Get a specific listing",
            "POST /listings": "Create a new listing",
            "POST /listings/:id/cancel": "Cancel a listing",
            "POST /listings/:id/purchase": "Purchase a listing",
            "GET /fees/calculate": "Calculate listing fees",
        },
        "documentation": "https://docs.1satordinals.com/public-apis",
        "powered_by": "GorillaPool 1Sat API"
    });
    Json(info)
}

/// Health check endpoint
pub async fn health(State(state): State<AppState>) -> impl IntoResponse {
    let uptime = state.start_time.elapsed().as_secs();
    let cache_stats = state.cache.stats();
    let listings_count = state.listings_db.count_active_listings();
    
    Json(HealthCheck {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_seconds: uptime,
        cache_stats,
        listings_count,
    })
}

// ============================================================================
// Wallet Handlers
// ============================================================================

/// Get all ordinals for a wallet address
pub async fn get_wallet_ordinals(
    Path(address): Path<String>,
    Query(params): Query<RefreshParam>,
    State(state): State<AppState>,
) -> Result<Json<WalletResponse>, (StatusCode, Json<ApiError>)> {
    info!("Wallet lookup request: {} (refresh={})", address, params.refresh);
    
    if address.is_empty() || address.len() < 20 || address.len() > 64 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError::new("invalid_address", "Address format is invalid")),
        ));
    }

    let result = if params.refresh {
        state.ordinal_service.refresh_wallet(&address).await
    } else {
        state.ordinal_service.get_wallet_ordinals(&address).await
    };

    match result {
        Ok(wallet_data) => {
            info!(
                "Wallet {} has {} ordinals (fetched in {}ms)",
                address, wallet_data.total_count, wallet_data.fetch_time_ms
            );
            Ok(Json(WalletResponse { success: true, data: wallet_data }))
        }
        Err(e) => {
            error!("Failed to fetch wallet ordinals: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("fetch_error", "Failed to fetch ordinals").with_details(e.to_string())),
            ))
        }
    }
}

/// Get ordinal details
pub async fn get_ordinal_details(
    Path(origin): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<OrdinalResponse>, (StatusCode, Json<ApiError>)> {
    info!("Ordinal details request: {}", origin);
    
    if !origin.contains('_') || origin.len() < 65 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError::new("invalid_origin", "Origin should be in format: txid_vout")),
        ));
    }

    match state.ordinal_service.get_ordinal_details(&origin).await {
        Ok(Some(details)) => Ok(Json(OrdinalResponse { success: true, data: details })),
        Ok(None) => Err((StatusCode::NOT_FOUND, Json(ApiError::new("not_found", "Ordinal not found")))),
        Err(e) => {
            error!("Failed to fetch ordinal details: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("fetch_error", "Failed to fetch ordinal details").with_details(e.to_string())),
            ))
        }
    }
}

/// Get ordinal content
pub async fn get_ordinal_content(
    Path(origin): Path<String>,
    State(state): State<AppState>,
) -> Result<Response, (StatusCode, Json<ApiError>)> {
    info!("Content request: {}", origin);

    match state.ordinal_service.get_ordinal_content(&origin).await {
        Ok((content, content_type)) => {
            Ok((
                StatusCode::OK,
                [
                    (header::CONTENT_TYPE, content_type),
                    (header::CACHE_CONTROL, "public, max-age=86400".to_string()),
                ],
                content,
            ).into_response())
        }
        Err(e) => {
            error!("Failed to fetch ordinal content: {}", e);
            Err((
                StatusCode::NOT_FOUND,
                Json(ApiError::new("content_error", "Failed to fetch content").with_details(e.to_string())),
            ))
        }
    }
}

// ============================================================================
// Listings Handlers
// ============================================================================

/// Calculate fees for a listing
#[derive(Debug, Deserialize)]
pub struct FeeCalcQuery {
    pub amount: u64,
    #[serde(default)]
    pub tip_percent: f64,
}

pub async fn calculate_fees(
    Query(params): Query<FeeCalcQuery>,
) -> Result<Json<FeeCalculationResponse>, (StatusCode, Json<ApiError>)> {
    let fees = ListingFees::calculate(params.amount, params.tip_percent);
    Ok(Json(FeeCalculationResponse { success: true, fees }))
}

/// Get active listings
pub async fn get_listings(
    Query(params): Query<ListingsQuery>,
    State(state): State<AppState>,
) -> Result<Json<ListingsResponse>, (StatusCode, Json<ApiError>)> {
    info!("Get listings: page={}, per_page={}", params.page, params.per_page);

    // If seller filter provided, get by seller
    if let Some(ref seller) = params.seller {
        match state.listings_db.get_listings_by_seller(seller) {
            Ok(listings) => {
                let total = listings.len();
                Ok(Json(ListingsResponse {
                    success: true,
                    listings,
                    total,
                    page: 1,
                    per_page: total,
                }))
            }
            Err(e) => {
                error!("Failed to get listings by seller: {}", e);
                Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiError::new("db_error", "Failed to fetch listings")),
                ))
            }
        }
    } else {
        match state.listings_db.get_active_listings(params.page, params.per_page) {
            Ok((listings, total)) => {
                Ok(Json(ListingsResponse {
                    success: true,
                    listings,
                    total,
                    page: params.page,
                    per_page: params.per_page,
                }))
            }
            Err(e) => {
                error!("Failed to get listings: {}", e);
                Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiError::new("db_error", "Failed to fetch listings")),
                ))
            }
        }
    }
}

/// Get a specific listing
pub async fn get_listing(
    Path(id): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
    match state.listings_db.get_listing(&id) {
        Ok(Some(listing)) => {
            Ok(Json(serde_json::json!({
                "success": true,
                "listing": listing
            })))
        }
        Ok(None) => {
            Err((StatusCode::NOT_FOUND, Json(ApiError::new("not_found", "Listing not found"))))
        }
        Err(e) => {
            error!("Failed to get listing: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("db_error", "Failed to fetch listing")),
            ))
        }
    }
}

/// Create a new listing
pub async fn create_listing(
    State(state): State<AppState>,
    Json(request): Json<CreateListingRequest>,
) -> Result<Json<CreateListingResponse>, (StatusCode, Json<ApiError>)> {
    info!("Create listing request for origin: {}", request.origin);

    // Check if already listed
    match state.listings_db.is_origin_listed(&request.origin) {
        Ok(true) => {
            return Err((
                StatusCode::CONFLICT,
                Json(ApiError::new("already_listed", "This ordinal is already listed")),
            ));
        }
        Err(e) => {
            error!("Failed to check listing: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("db_error", "Database error")),
            ));
        }
        _ => {}
    }

    // Validate tip percent
    if request.tip_percent != 0.0 && request.tip_percent != 2.5 && request.tip_percent != 5.0 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError::new("invalid_tip", "Tip must be 0%, 2.5%, or 5%")),
        ));
    }

    // Create listing
    match state.listings_db.create_listing(request) {
        Ok(listing) => {
            info!("Created listing {}", listing.id);
            Ok(Json(CreateListingResponse {
                success: true,
                listing,
                message: "Listing created successfully".to_string(),
            }))
        }
        Err(e) => {
            error!("Failed to create listing: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("create_error", "Failed to create listing").with_details(e.to_string())),
            ))
        }
    }
}

/// Cancel a listing
pub async fn cancel_listing(
    Path(id): Path<String>,
    State(state): State<AppState>,
    Json(request): Json<CancelListingRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
    info!("Cancel listing request: {}", id);

    if id != request.listing_id {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError::new("id_mismatch", "Listing ID mismatch")),
        ));
    }

    match state.listings_db.cancel_listing(&id, &request.seller_ord_address) {
        Ok(Some(listing)) => {
            Ok(Json(serde_json::json!({
                "success": true,
                "listing": listing,
                "message": "Listing cancelled successfully"
            })))
        }
        Ok(None) => {
            Err((StatusCode::NOT_FOUND, Json(ApiError::new("not_found", "Listing not found"))))
        }
        Err(e) => {
            error!("Failed to cancel listing: {}", e);
            Err((
                StatusCode::BAD_REQUEST,
                Json(ApiError::new("cancel_error", e.to_string())),
            ))
        }
    }
}

/// Purchase a listing (placeholder for now - actual implementation needs PSBT handling)
pub async fn purchase_listing(
    Path(id): Path<String>,
    State(state): State<AppState>,
    Json(request): Json<PurchaseListingRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
    info!("Purchase listing request: {}", id);

    if id != request.listing_id {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError::new("id_mismatch", "Listing ID mismatch")),
        ));
    }

    // Get the listing
    let listing = match state.listings_db.get_listing(&id) {
        Ok(Some(l)) => l,
        Ok(None) => {
            return Err((StatusCode::NOT_FOUND, Json(ApiError::new("not_found", "Listing not found"))));
        }
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("db_error", e.to_string())),
            ));
        }
    };

    // For now, return the listing info needed to complete the purchase client-side
    // In production, you'd construct the full transaction here
    Ok(Json(serde_json::json!({
        "success": true,
        "listing": listing,
        "message": "Purchase ready - complete transaction client-side",
        "purchase_info": {
            "total_satoshis": listing.fees.total_price,
            "seller_receives": listing.fees.seller_receives,
            "marketplace_fee": listing.fees.marketplace_fee,
            "tip_amount": listing.fees.tip_amount,
            "seller_address": listing.seller_address,
            "ordinal_utxo": listing.ordinal_utxo,
        }
    })))
}

/// Get listing by origin
pub async fn get_listing_by_origin(
    Path(origin): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
    match state.listings_db.get_listing_by_origin(&origin) {
        Ok(Some(listing)) => {
            Ok(Json(serde_json::json!({
                "success": true,
                "listed": true,
                "listing": listing
            })))
        }
        Ok(None) => {
            Ok(Json(serde_json::json!({
                "success": true,
                "listed": false,
                "listing": null
            })))
        }
        Err(e) => {
            error!("Failed to get listing by origin: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("db_error", "Failed to fetch listing")),
            ))
        }
    }
}

// ============================================================================
// Search (placeholder)
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct SearchParams {
    pub content_type: Option<String>,
    pub collection_id: Option<String>,
    #[serde(default = "default_page")]
    pub page: usize,
    #[serde(default = "default_per_page")]
    pub per_page: usize,
}

fn default_page() -> usize { 1 }
fn default_per_page() -> usize { 50 }

pub async fn search_ordinals(
    Query(_params): Query<SearchParams>,
    State(_state): State<AppState>,
) -> impl IntoResponse {
    Json(serde_json::json!({
        "error": "not_implemented",
        "message": "Search functionality coming soon"
    }))
}
