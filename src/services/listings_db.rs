use crate::models::{Listing, ListingStatus, ListingFees, CreateListingRequest, OrdinalUtxoRef};
use anyhow::{Context, Result};
use chrono::Utc;
use sled::Db;
use std::sync::Arc;
use tracing::{debug, info, error};
use uuid::Uuid;

/// Listings database manager
pub struct ListingsDb {
    db: Arc<Db>,
}

impl ListingsDb {
    pub fn new(db: Arc<Db>) -> Self {
        Self { db }
    }

    /// Create a new listing
    pub fn create_listing(&self, request: CreateListingRequest) -> Result<Listing> {
        // Validate tip percent
        let tip_percent = match request.tip_percent {
            p if p == 0.0 => 0.0,
            p if (p - 2.5).abs() < 0.01 => 2.5,
            p if (p - 5.0).abs() < 0.01 => 5.0,
            _ => 0.0, // Default to 0 if invalid
        };

        // Calculate fees
        let fees = ListingFees::calculate(request.seller_wants_satoshis, tip_percent);

        let listing = Listing {
            id: Uuid::new_v4().to_string(),
            origin: request.origin.clone(),
            seller_address: request.seller_address,
            seller_ord_address: request.seller_ord_address,
            fees,
            status: ListingStatus::Active,
            psbt_hex: None,
            listing_utxo: None,
            ordinal_utxo: request.ordinal_utxo,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            sold_at: None,
            buyer_address: None,
            purchase_txid: None,
        };

        // Store in database
        let key = format!("listing:{}", listing.id);
        let value = serde_json::to_vec(&listing).context("Failed to serialize listing")?;
        self.db.insert(key.as_bytes(), value).context("Failed to insert listing")?;

        // Also index by origin for quick lookup
        let origin_key = format!("listing_by_origin:{}", listing.origin);
        self.db.insert(origin_key.as_bytes(), listing.id.as_bytes())
            .context("Failed to insert origin index")?;

        // Index by seller
        let seller_key = format!("listing_by_seller:{}:{}", listing.seller_address, listing.id);
        self.db.insert(seller_key.as_bytes(), listing.id.as_bytes())
            .context("Failed to insert seller index")?;

        info!("Created listing {} for origin {} at {} sats", listing.id, listing.origin, listing.fees.total_price);
        
        Ok(listing)
    }

    /// Get a listing by ID
    pub fn get_listing(&self, id: &str) -> Result<Option<Listing>> {
        let key = format!("listing:{}", id);
        
        match self.db.get(key.as_bytes())? {
            Some(bytes) => {
                let listing: Listing = serde_json::from_slice(&bytes)
                    .context("Failed to deserialize listing")?;
                Ok(Some(listing))
            }
            None => Ok(None),
        }
    }

    /// Get a listing by origin
    pub fn get_listing_by_origin(&self, origin: &str) -> Result<Option<Listing>> {
        let origin_key = format!("listing_by_origin:{}", origin);
        
        match self.db.get(origin_key.as_bytes())? {
            Some(id_bytes) => {
                let id = String::from_utf8_lossy(&id_bytes);
                self.get_listing(&id)
            }
            None => Ok(None),
        }
    }

    /// Update a listing
    pub fn update_listing(&self, listing: &Listing) -> Result<()> {
        let key = format!("listing:{}", listing.id);
        let value = serde_json::to_vec(listing).context("Failed to serialize listing")?;
        self.db.insert(key.as_bytes(), value).context("Failed to update listing")?;
        
        debug!("Updated listing {}", listing.id);
        Ok(())
    }

    /// Cancel a listing
    pub fn cancel_listing(&self, id: &str, seller_ord_address: &str) -> Result<Option<Listing>> {
        let mut listing = match self.get_listing(id)? {
            Some(l) => l,
            None => return Ok(None),
        };

        // Verify seller
        if listing.seller_ord_address != seller_ord_address {
            anyhow::bail!("Not authorized to cancel this listing");
        }

        // Verify status
        if listing.status != ListingStatus::Active {
            anyhow::bail!("Listing is not active");
        }

        // Update status
        listing.status = ListingStatus::Cancelled;
        listing.updated_at = Utc::now();
        
        self.update_listing(&listing)?;

        // Remove from origin index
        let origin_key = format!("listing_by_origin:{}", listing.origin);
        self.db.remove(origin_key.as_bytes())?;

        info!("Cancelled listing {}", id);
        Ok(Some(listing))
    }

    /// Mark a listing as sold
    pub fn mark_listing_sold(
        &self, 
        id: &str, 
        buyer_address: &str,
        purchase_txid: &str
    ) -> Result<Option<Listing>> {
        let mut listing = match self.get_listing(id)? {
            Some(l) => l,
            None => return Ok(None),
        };

        if listing.status != ListingStatus::Active {
            anyhow::bail!("Listing is not active");
        }

        listing.status = ListingStatus::Sold;
        listing.sold_at = Some(Utc::now());
        listing.buyer_address = Some(buyer_address.to_string());
        listing.purchase_txid = Some(purchase_txid.to_string());
        listing.updated_at = Utc::now();

        self.update_listing(&listing)?;

        // Remove from origin index
        let origin_key = format!("listing_by_origin:{}", listing.origin);
        self.db.remove(origin_key.as_bytes())?;

        info!("Listing {} sold to {} in tx {}", id, buyer_address, purchase_txid);
        Ok(Some(listing))
    }

    /// Get all active listings
    pub fn get_active_listings(&self, page: usize, per_page: usize) -> Result<(Vec<Listing>, usize)> {
        let mut listings = Vec::new();
        
        for item in self.db.scan_prefix(b"listing:") {
            if let Ok((_, value)) = item {
                if let Ok(listing) = serde_json::from_slice::<Listing>(&value) {
                    if listing.status == ListingStatus::Active {
                        listings.push(listing);
                    }
                }
            }
        }

        // Sort by created_at descending
        listings.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        
        let total = listings.len();
        
        // Paginate
        let start = (page - 1) * per_page;
        let paginated: Vec<Listing> = listings
            .into_iter()
            .skip(start)
            .take(per_page)
            .collect();

        Ok((paginated, total))
    }

    /// Get listings by seller
    pub fn get_listings_by_seller(&self, seller_address: &str) -> Result<Vec<Listing>> {
        let prefix = format!("listing_by_seller:{}:", seller_address);
        let mut listings = Vec::new();
        
        for item in self.db.scan_prefix(prefix.as_bytes()) {
            if let Ok((_, id_bytes)) = item {
                let id = String::from_utf8_lossy(&id_bytes);
                if let Ok(Some(listing)) = self.get_listing(&id) {
                    listings.push(listing);
                }
            }
        }

        listings.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(listings)
    }

    /// Count active listings
    pub fn count_active_listings(&self) -> usize {
        let mut count = 0;
        for item in self.db.scan_prefix(b"listing:") {
            if let Ok((_, value)) = item {
                if let Ok(listing) = serde_json::from_slice::<Listing>(&value) {
                    if listing.status == ListingStatus::Active {
                        count += 1;
                    }
                }
            }
        }
        count
    }

    /// Check if an origin is already listed
    pub fn is_origin_listed(&self, origin: &str) -> Result<bool> {
        Ok(self.get_listing_by_origin(origin)?.is_some())
    }
}

impl Clone for ListingsDb {
    fn clone(&self) -> Self {
        Self {
            db: Arc::clone(&self.db),
        }
    }
}
