use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// UTXO with ordinal data from GorillaPool API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrdinalUtxo {
    pub txid: String,
    pub vout: u32,
    pub satoshis: u64,
    pub lock: String,
    pub origin: String,
    #[serde(default)]
    pub ordinal: u64,
    #[serde(default)]
    pub spend: Option<String>,
}

/// Inscription data from GorillaPool API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Inscription {
    #[serde(default)]
    pub id: Option<u64>,
    pub txid: String,
    pub vout: u32,
    #[serde(default)]
    pub file: Option<InscriptionFile>,
    pub origin: String,
    #[serde(default)]
    pub ordinal: u64,
    #[serde(default)]
    pub height: Option<u64>,
    #[serde(default)]
    pub idx: Option<u64>,
    #[serde(default)]
    pub lock: Option<String>,
    #[serde(default)]
    pub map: Option<serde_json::Value>,
    #[serde(default)]
    pub b: Option<serde_json::Value>,
    #[serde(default)]
    pub sigma: Option<Vec<serde_json::Value>>,
}

/// File information within an inscription
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InscriptionFile {
    pub hash: String,
    pub size: u64,
    #[serde(rename = "type")]
    pub content_type: String,
}

/// Extended ordinal data combining UTXO + inscription details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrdinalDetails {
    pub origin: String,
    pub txid: String,
    pub vout: u32,
    pub owner_address: String,
    pub satoshis: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_height: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inscription_number: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collection_id: Option<String>,
    pub content_url: String,
    pub preview_url: String,
    pub fetched_at: DateTime<Utc>,
}

/// Wallet summary with all ordinals
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletOrdinals {
    pub address: String,
    pub total_count: usize,
    pub ordinals: Vec<OrdinalDetails>,
    pub fetched_at: DateTime<Utc>,
    pub fetch_time_ms: u64,
}

/// API error response
#[derive(Debug, Serialize)]
pub struct ApiError {
    pub error: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

impl ApiError {
    pub fn new(error: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            error: error.into(),
            message: message.into(),
            details: None,
        }
    }
    
    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(details.into());
        self
    }
}

/// Health check response
#[derive(Debug, Serialize)]
pub struct HealthCheck {
    pub status: String,
    pub version: String,
    pub uptime_seconds: u64,
    pub cache_stats: CacheStats,
    pub listings_count: usize,
}

/// Cache statistics
#[derive(Debug, Serialize, Default)]
pub struct CacheStats {
    pub ownership_entries: u64,
    pub content_entries: u64,
    pub hit_rate_percent: f64,
}

// =============================================================================
// Marketplace Listing Models
// =============================================================================

/// Listing status
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ListingStatus {
    Active,
    Sold,
    Cancelled,
}

/// Fee breakdown for a listing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListingFees {
    /// Price the seller wants to receive (in satoshis)
    pub seller_receives: u64,
    /// Marketplace fee (1%) in satoshis
    pub marketplace_fee: u64,
    /// Optional tip to the platform (in satoshis)
    pub tip_amount: u64,
    /// Tip percentage (0, 2.5, or 5)
    pub tip_percent: f64,
    /// Total price buyer pays (in satoshis)
    pub total_price: u64,
}

impl ListingFees {
    pub fn calculate(seller_wants: u64, tip_percent: f64) -> Self {
        // Marketplace fee is 1% of what seller wants
        let marketplace_fee = (seller_wants as f64 * 0.01).ceil() as u64;
        
        // Tip is percentage of seller_wants
        let tip_amount = (seller_wants as f64 * (tip_percent / 100.0)).ceil() as u64;
        
        // What seller actually receives
        let seller_receives = seller_wants;
        
        // Total buyer pays
        let total_price = seller_receives + marketplace_fee + tip_amount;
        
        Self {
            seller_receives,
            marketplace_fee,
            tip_amount,
            tip_percent,
            total_price,
        }
    }
}

/// A marketplace listing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Listing {
    /// Unique listing ID
    pub id: String,
    /// Ordinal origin (txid_vout)
    pub origin: String,
    /// Seller's BSV address (receives payment)
    pub seller_address: String,
    /// Seller's ordinal address (for cancellation)
    pub seller_ord_address: String,
    /// Fee breakdown
    pub fees: ListingFees,
    /// Listing status
    pub status: ListingStatus,
    /// PSBT hex for the listing (partially signed transaction)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub psbt_hex: Option<String>,
    /// The listing UTXO (txid:vout of the ordinal lock output)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub listing_utxo: Option<String>,
    /// Original ordinal UTXO being listed
    pub ordinal_utxo: OrdinalUtxoRef,
    /// When the listing was created
    pub created_at: DateTime<Utc>,
    /// When the listing was updated
    pub updated_at: DateTime<Utc>,
    /// When the listing was sold (if sold)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sold_at: Option<DateTime<Utc>>,
    /// Buyer address (if sold)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub buyer_address: Option<String>,
    /// Purchase transaction ID (if sold)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub purchase_txid: Option<String>,
}

/// Reference to an ordinal UTXO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrdinalUtxoRef {
    pub txid: String,
    pub vout: u32,
    pub satoshis: u64,
    pub script: String,  // Base64 encoded
}

/// Request to create a new listing
#[derive(Debug, Deserialize)]
pub struct CreateListingRequest {
    /// Ordinal origin to list
    pub origin: String,
    /// The UTXO containing the ordinal
    pub ordinal_utxo: OrdinalUtxoRef,
    /// What the seller wants to receive (in satoshis)
    pub seller_wants_satoshis: u64,
    /// Tip percentage (0, 2.5, or 5)
    #[serde(default)]
    pub tip_percent: f64,
    /// Seller's BSV address (to receive payment)
    pub seller_address: String,
    /// Seller's ordinal address (for cancellation return)
    pub seller_ord_address: String,
}

/// Response when creating a listing
#[derive(Debug, Serialize)]
pub struct CreateListingResponse {
    pub success: bool,
    pub listing: Listing,
    /// Message for the user
    pub message: String,
}

/// Request to cancel a listing
#[derive(Debug, Deserialize)]
pub struct CancelListingRequest {
    pub listing_id: String,
    pub seller_ord_address: String,
}

/// Request to purchase a listing
#[derive(Debug, Deserialize)]
pub struct PurchaseListingRequest {
    pub listing_id: String,
    pub buyer_address: String,
    pub buyer_ord_address: String,
    /// UTXOs to fund the purchase
    pub payment_utxos: Vec<OrdinalUtxoRef>,
}

/// Paginated listings response
#[derive(Debug, Serialize)]
pub struct ListingsResponse {
    pub success: bool,
    pub listings: Vec<Listing>,
    pub total: usize,
    pub page: usize,
    pub per_page: usize,
}

/// Query parameters for listing listings
#[derive(Debug, Deserialize)]
pub struct ListingsQuery {
    #[serde(default = "default_page")]
    pub page: usize,
    #[serde(default = "default_per_page")]
    pub per_page: usize,
    /// Filter by seller address
    pub seller: Option<String>,
    /// Filter by status
    pub status: Option<String>,
}

fn default_page() -> usize { 1 }
fn default_per_page() -> usize { 50 }
