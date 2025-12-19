// =================================================================
// 4chain Ordinals Marketplace - All-in-One JavaScript
// =================================================================

(function() {
    'use strict';

    // =================================================================
    // Configuration
    // =================================================================
    const CONFIG = {
        API_BASE: 'http://localhost:3000',
        GORILLA_API: 'https://ordinals.gorillapool.io/api',
        MARKETPLACE_FEE_ADDRESS: '1YourMarketplaceFeeAddressHere',
        MARKETPLACE_FEE_PERCENT: 1,
        TIP_OPTIONS: [0, 2.5, 5],
        HEALTH_CHECK_INTERVAL: 30000,
        WALLET_DETECT_TIMEOUT: 3000
    };

    // =================================================================
    // State Management
    // =================================================================
    const state = {
        currentOrdinals: [],
        myOrdinals: [],
        marketplaceListings: [],
        walletConnected: false,
        walletAddress: null,
        walletOrdAddress: null,
        selectedTipPercent: 0
    };

    // =================================================================
    // Utility Functions
    // =================================================================

    function truncateAddress(addr) {
        if (!addr || addr.length < 15) return addr || '--';
        return `${addr.slice(0, 6)}...${addr.slice(-6)}`;
    }

    function truncateOrigin(origin) {
        if (!origin || origin.length < 20) return origin;
        return `${origin.slice(0, 10)}...${origin.slice(-6)}`;
    }

    function formatSize(bytes) {
        if (!bytes) return 'N/A';
        if (bytes < 1024) return `${bytes} B`;
        if (bytes < 1048576) return `${(bytes / 1024).toFixed(1)} KB`;
        return `${(bytes / 1048576).toFixed(2)} MB`;
    }

    function satsToBsv(sats) {
        if (!sats || sats < 0) return '0';
        return (sats / 100000000).toFixed(8).replace(/\.?0+$/, '') || '0';
    }

    function bsvToSats(bsv) {
        return Math.round(parseFloat(bsv) * 100000000);
    }

    function extractName(ordinal) {
        return ordinal.metadata?.name || ordinal.metadata?.subTypeData?.name || null;
    }

    function escapeHtml(text) {
        if (!text) return '';
        const div = document.createElement('div');
        div.textContent = text;
        return div.innerHTML;
    }

    function copyToClipboard(text) {
        navigator.clipboard.writeText(text).then(() => {
            showToast('Copied!', 'success');
        }).catch(err => {
            console.error('Copy failed:', err);
            showToast('Failed to copy', 'error');
        });
    }

    function getContentUrl(ordinal) {
        if (ordinal.origin) {
            return `${CONFIG.GORILLA_API}/files/inscriptions/${ordinal.origin}`;
        }
        if (ordinal.txid && ordinal.vout !== undefined) {
            return `${CONFIG.GORILLA_API}/files/inscriptions/${ordinal.txid}_${ordinal.vout}`;
        }
        return null;
    }

    // =================================================================
    // Toast Notifications
    // =================================================================

    function showToast(message, type = 'info') {
        const container = document.getElementById('toastContainer');
        if (!container) return;
        
        const toast = document.createElement('div');
        toast.className = `toast ${type}`;
        
        const icon = type === 'success' ? '✓' : type === 'error' ? '✗' : 'ℹ';
        toast.innerHTML = `<span>${icon}</span><span>${escapeHtml(message)}</span>`;
        
        container.appendChild(toast);
        
        setTimeout(() => {
            toast.style.opacity = '0';
            toast.style.transform = 'translateX(50px)';
            setTimeout(() => toast.remove(), 300);
        }, 3500);
    }

    // =================================================================
    // Modal Functions
    // =================================================================

    function closeModal(event) {
        const overlay = document.getElementById('modalOverlay');
        if (!event || event.target === overlay) {
            overlay.classList.remove('active');
        }
    }

    // =================================================================
    // Tab Switching
    // =================================================================

    function switchTab(tabName) {
        document.querySelectorAll('.tab').forEach(t => t.classList.remove('active'));
        document.querySelectorAll('.tab-content').forEach(c => c.classList.remove('active'));
        
        if (event && event.target) {
            event.target.classList.add('active');
        }
        document.getElementById(`${tabName}-tab`).classList.add('active');
        
        if (tabName === 'marketplace') {
            loadMarketplaceListings();
        }
    }

    // =================================================================
    // Fee Calculations
    // =================================================================

    function calculateFees(listingPriceSats, tipPercent = 0) {
        const marketplaceFeeSats = Math.ceil(listingPriceSats * (CONFIG.MARKETPLACE_FEE_PERCENT / 100));
        const tipSats = Math.ceil(listingPriceSats * (tipPercent / 100));
        const sellerReceivesSats = listingPriceSats - marketplaceFeeSats - tipSats;
        
        return {
            listingPrice: listingPriceSats,
            marketplaceFee: marketplaceFeeSats,
            tip: tipSats,
            sellerReceives: Math.max(0, sellerReceivesSats)
        };
    }

    function selectTip(percent) {
        state.selectedTipPercent = percent;
        
        document.querySelectorAll('.tip-btn').forEach(btn => {
            btn.classList.remove('active');
        });
        if (event && event.target) {
            event.target.classList.add('active');
        }
        
        updateFeeBreakdown();
    }

    function updateFeeBreakdown() {
        const priceInput = document.getElementById('listingPrice');
        if (!priceInput) return;
        
        const listingPriceBsv = parseFloat(priceInput.value) || 0;
        const listingPriceSats = bsvToSats(listingPriceBsv);
        
        const fees = calculateFees(listingPriceSats, state.selectedTipPercent);
        
        const elements = {
            listingPriceDisplay: document.getElementById('listingPriceDisplay'),
            marketplaceFee: document.getElementById('marketplaceFee'),
            tipAmount: document.getElementById('tipAmount'),
            youReceive: document.getElementById('youReceive')
        };
        
        if (elements.listingPriceDisplay) {
            elements.listingPriceDisplay.textContent = satsToBsv(fees.listingPrice) + ' BSV';
        }
        if (elements.marketplaceFee) {
            elements.marketplaceFee.textContent = '-' + satsToBsv(fees.marketplaceFee) + ' BSV';
        }
        if (elements.tipAmount) {
            elements.tipAmount.textContent = '-' + satsToBsv(fees.tip) + ' BSV';
        }
        if (elements.youReceive) {
            elements.youReceive.textContent = satsToBsv(fees.sellerReceives) + ' BSV';
        }
    }

    // =================================================================
    // Wallet Integration
    // =================================================================

    function waitForYoursWallet(timeout = CONFIG.WALLET_DETECT_TIMEOUT) {
        return new Promise((resolve) => {
            if (window.yours) {
                resolve(true);
                return;
            }
            
            const start = Date.now();
            const check = setInterval(() => {
                if (window.yours) {
                    clearInterval(check);
                    resolve(true);
                } else if (Date.now() - start > timeout) {
                    clearInterval(check);
                    resolve(false);
                }
            }, 100);
        });
    }

    function checkYoursWallet() {
        setTimeout(async () => {
            const hasWallet = await waitForYoursWallet(1000);
            if (hasWallet) {
                try {
                    const connected = await window.yours.isConnected();
                    if (connected) {
                        await handleWalletConnected();
                    }
                } catch (err) {
                    console.error('Wallet check error:', err);
                }
            }
        }, 500);
    }

    async function connectWallet() {
        const hasWallet = await waitForYoursWallet();
        
        if (!hasWallet) {
            showToast('Yours Wallet not detected', 'error');
            window.open('https://yours.org', '_blank');
            return;
        }

        const btn = document.getElementById('connectWalletBtn');
        
        try {
            btn.disabled = true;
            btn.textContent = 'Connecting...';

            const connected = await window.yours.connect();
            
            if (connected) {
                await handleWalletConnected();
            } else {
                throw new Error('Connection rejected');
            }
        } catch (error) {
            console.error('Wallet connection error:', error);
            showToast('Failed: ' + error.message, 'error');
            btn.textContent = 'Connect Wallet';
        } finally {
            btn.disabled = false;
        }
    }

    async function handleWalletConnected() {
        try {
            state.walletConnected = true;
            
            const addresses = await window.yours.getAddresses();
            console.log('Wallet addresses:', addresses);
            
            state.walletAddress = addresses.bsvAddress;
            state.walletOrdAddress = addresses.ordAddress;
            
            let bsvBalance = 0;
            try {
                const balance = await window.yours.getBalance();
                console.log('Wallet balance:', balance);
                
                if (balance?.bsv?.satoshis !== undefined) {
                    bsvBalance = balance.bsv.satoshis / 100000000;
                } else if (balance?.satoshis !== undefined) {
                    bsvBalance = balance.satoshis / 100000000;
                } else if (typeof balance === 'number') {
                    bsvBalance = balance / 100000000;
                }
            } catch (e) {
                console.log('Balance fetch error:', e);
            }
            
            updateWalletUI(bsvBalance);
            showToast('Wallet connected!', 'success');
            await loadMyOrdinals();
            
        } catch (error) {
            console.error('Wallet setup error:', error);
            showToast('Error: ' + error.message, 'error');
        }
    }

    function updateWalletUI(bsvBalance) {
        const walletInfo = document.getElementById('walletInfo');
        const walletAddressEl = document.getElementById('walletAddress');
        const walletBalanceEl = document.getElementById('walletBalance');
        const connectBtn = document.getElementById('connectWalletBtn');
        const walletStatusText = document.getElementById('walletStatusText');
        
        walletInfo.classList.add('connected');
        
        walletAddressEl.textContent = truncateAddress(state.walletOrdAddress);
        walletAddressEl.title = `Ord: ${state.walletOrdAddress}\nBSV: ${state.walletAddress}`;
        walletAddressEl.onclick = () => copyToClipboard(state.walletOrdAddress);
        
        walletBalanceEl.textContent = `${bsvBalance.toFixed(4)} BSV`;
        
        connectBtn.textContent = 'Connected ✓';
        connectBtn.classList.remove('btn-wallet');
        connectBtn.classList.add('btn-secondary');
        
        walletStatusText.textContent = `Ord: ${truncateAddress(state.walletOrdAddress)}`;
    }

    // =================================================================
    // Ordinals Display & Search
    // =================================================================

    async function loadMyOrdinals() {
        if (!state.walletOrdAddress) return;
        
        const container = document.getElementById('myOrdinalsContent');
        container.innerHTML = `
            <div class="loading">
                <div class="spinner"></div>
                <div>Loading your ordinals...</div>
            </div>
        `;

        try {
            const response = await fetch(`${CONFIG.API_BASE}/wallet/${state.walletOrdAddress}`);
            const data = await response.json();
            
            if (data.success && data.data) {
                state.myOrdinals = data.data.ordinals;
                
                if (state.myOrdinals.length >= 100) {
                    showToast('Showing first 100 ordinals', 'info');
                }
                
                renderMyOrdinals();
            } else {
                throw new Error(data.message || 'Failed to load');
            }
        } catch (error) {
            console.error('Load ordinals error:', error);
            container.innerHTML = `
                <div class="empty-state">
                    <div class="empty-icon">⚠️</div>
                    <h3 class="empty-title">Error</h3>
                    <p>${escapeHtml(error.message)}</p>
                    <button class="btn btn-secondary" style="margin-top:12px;" onclick="loadMyOrdinals()">Retry</button>
                </div>
            `;
        }
    }

    function renderMyOrdinals() {
        const container = document.getElementById('myOrdinalsContent');
        
        if (!state.myOrdinals.length) {
            container.innerHTML = `
                <div class="empty-state">
                    <div class="empty-icon">◇</div>
                    <h3 class="empty-title">No Ordinals</h3>
                    <p>Your wallet is empty</p>
                </div>
            `;
            return;
        }

        let html = `
            <div class="stats-bar">
                <div class="stat-card">
                    <div class="stat-value">${state.myOrdinals.length}</div>
                    <div class="stat-label">My Ordinals</div>
                </div>
            </div>
            <div class="ordinals-grid">
        `;

        state.myOrdinals.forEach(ord => {
            const isListed = state.marketplaceListings.some(l => l.origin === ord.origin);
            html += createOrdinalCard(ord, true, isListed);
        });

        html += '</div>';
        container.innerHTML = html;
    }

    async function searchWallet() {
        const address = document.getElementById('addressInput').value.trim();
        
        if (!address) {
            showToast('Enter an address', 'error');
            return;
        }

        const container = document.getElementById('exploreResults');
        const statsBar = document.getElementById('statsBar');
        const searchBtn = document.getElementById('searchBtn');
        
        container.innerHTML = `
            <div class="loading">
                <div class="spinner"></div>
                <div>Fetching ordinals...</div>
            </div>
        `;
        statsBar.style.display = 'none';
        searchBtn.disabled = true;

        try {
            const response = await fetch(`${CONFIG.API_BASE}/wallet/${encodeURIComponent(address)}`);
            const data = await response.json();

            if (data.success && data.data) {
                state.currentOrdinals = data.data.ordinals;
                displaySearchResults(data.data);
            } else {
                throw new Error(data.message || 'Failed to fetch');
            }
        } catch (error) {
            console.error('Search error:', error);
            container.innerHTML = `
                <div class="empty-state">
                    <div class="empty-icon">⚠️</div>
                    <h3 class="empty-title">Error</h3>
                    <p>${escapeHtml(error.message)}</p>
                </div>
            `;
        } finally {
            searchBtn.disabled = false;
        }
    }

    function displaySearchResults(data) {
        const container = document.getElementById('exploreResults');
        const statsBar = document.getElementById('statsBar');
        
        if (!data.ordinals?.length) {
            container.innerHTML = `
                <div class="empty-state">
                    <div class="empty-icon">◇</div>
                    <h3 class="empty-title">No Ordinals</h3>
                    <p>Wallet is empty</p>
                </div>
            `;
            return;
        }

        statsBar.style.display = 'grid';
        document.getElementById('totalOrdinals').textContent = data.total_count;
        document.getElementById('fetchTime').textContent = `${data.fetch_time_ms}ms`;
        document.getElementById('imageCount').textContent = 
            data.ordinals.filter(o => o.content_type?.startsWith('image/')).length;

        let html = '<div class="ordinals-grid">';
        data.ordinals.forEach(ord => {
            html += createOrdinalCard(ord, false, false);
        });
        html += '</div>';
        
        container.innerHTML = html;
    }

    function createOrdinalCard(ordinal, isOwned, isListed) {
        const isImage = ordinal.content_type?.startsWith('image/');
        const contentType = ordinal.content_type?.split('/')[1]?.toUpperCase() || 'FILE';
        const name = extractName(ordinal) || `#${ordinal.inscription_number || '?'}`;
        const imageUrl = getContentUrl(ordinal);
        const listing = state.marketplaceListings.find(l => l.origin === ordinal.origin);
        
        return `
            <div class="ordinal-card ${isListed ? 'listed' : ''}" onclick="showOrdinalDetails('${ordinal.origin}', ${isOwned})">
                <div class="ordinal-preview">
                    ${isImage && imageUrl
                        ? `<img src="${imageUrl}" alt="${escapeHtml(name)}" 
                                loading="eager" 
                                crossorigin="anonymous" 
                                onload="this.classList.add('loaded')"
                                onerror="this.style.display='none';this.nextElementSibling.style.display='flex';">
                           <div class="placeholder" style="display:none;">◇</div>`
                        : `<div class="placeholder">◇</div>`
                    }
                    <span class="badge badge-type">${contentType}</span>
                    ${isListed ? '<span class="badge badge-listed">Listed</span>' : ''}
                </div>
                <div class="ordinal-info">
                    <div class="ordinal-name">${escapeHtml(name)}</div>
                    <div class="ordinal-origin">${truncateOrigin(ordinal.origin)}</div>
                    ${listing ? `
                        <div class="ordinal-price">
                            <div class="price-value">${satsToBsv(listing.fees.total_price)} BSV</div>
                            <div class="price-label">Price</div>
                        </div>
                    ` : ''}
                </div>
            </div>
        `;
    }

    function showOrdinalDetails(origin, isOwned) {
        const ordinal = [...state.currentOrdinals, ...state.myOrdinals].find(o => o.origin === origin);
        if (!ordinal) {
            showToast('Ordinal not found', 'error');
            return;
        }

        const isImage = ordinal.content_type?.startsWith('image/');
        const listing = state.marketplaceListings.find(l => l.origin === origin);
        const isListed = !!listing;
        const imageUrl = getContentUrl(ordinal);
        const name = extractName(ordinal) || `#${ordinal.inscription_number || '?'}`;
        
        document.getElementById('modalTitle').textContent = name;
        state.selectedTipPercent = 0;
        
        let actionsHtml = buildActionsHtml(origin, isOwned, isListed, listing);

        document.getElementById('modalContent').innerHTML = `
            <div class="modal-preview">
                ${isImage && imageUrl 
                    ? `<img src="${imageUrl}" alt="${escapeHtml(name)}" crossorigin="anonymous" 
                            onerror="this.outerHTML='<div class=\\'placeholder\\' style=\\'font-size:3rem;padding:40px;\\'>◇</div>'">`
                    : `<div class="placeholder" style="font-size:3rem;padding:40px;">◇</div>`
                }
            </div>
            <div class="detail-grid">
                <div class="detail-item full-width">
                    <div class="detail-label">Origin</div>
                    <div class="detail-value" style="font-size:0.7rem;">${origin}</div>
                </div>
                <div class="detail-item">
                    <div class="detail-label">Type</div>
                    <div class="detail-value highlight">${ordinal.content_type || 'Unknown'}</div>
                </div>
                <div class="detail-item">
                    <div class="detail-label">Size</div>
                    <div class="detail-value">${formatSize(ordinal.content_size)}</div>
                </div>
                <div class="detail-item">
                    <div class="detail-label">Block</div>
                    <div class="detail-value">${ordinal.block_height || 'N/A'}</div>
                </div>
                <div class="detail-item">
                    <div class="detail-label">Owner</div>
                    <div class="detail-value">${truncateAddress(ordinal.owner_address)}</div>
                </div>
                <div class="detail-item full-width">
                    <div style="display:flex;gap:8px;flex-wrap:wrap;">
                        ${imageUrl ? `<a href="${imageUrl}" target="_blank" class="btn btn-secondary btn-small" style="text-decoration:none;">View Content</a>` : ''}
                        <button class="btn btn-secondary btn-small" onclick="copyToClipboard('${origin}')">Copy Origin</button>
                    </div>
                </div>
            </div>
            ${actionsHtml}
        `;

        document.getElementById('modalOverlay').classList.add('active');
    }

    function buildActionsHtml(origin, isOwned, isListed, listing) {
        if (isOwned && state.walletConnected && !isListed) {
            return `
                <div class="listing-form">
                    <div class="form-title">📝 Create Listing</div>
                    <div class="form-group">
                        <label class="form-label">Listing Price (BSV)</label>
                        <input type="number" class="form-input" id="listingPrice" 
                               placeholder="0.05" step="0.0001" min="0.0001" 
                               oninput="updateFeeBreakdown()">
                    </div>
                    <div class="form-group">
                        <label class="form-label">Support 4chain (Optional Tip)</label>
                        <div class="tip-options">
                            <button class="tip-btn active" onclick="selectTip(0)">No Tip</button>
                            <button class="tip-btn" onclick="selectTip(2.5)">2.5%</button>
                            <button class="tip-btn" onclick="selectTip(5)">5%</button>
                        </div>
                    </div>
                    <div class="fee-breakdown" id="feeBreakdown">
                        <div class="fee-row">
                            <span class="label">Listing Price</span>
                            <span class="value" id="listingPriceDisplay">0 BSV</span>
                        </div>
                        <div class="fee-row deduction">
                            <span class="label">- Platform Fee (1%)</span>
                            <span class="value" id="marketplaceFee">0 BSV</span>
                        </div>
                        <div class="fee-row deduction">
                            <span class="label">- Tip</span>
                            <span class="value" id="tipAmount">0 BSV</span>
                        </div>
                        <div class="fee-row total receives">
                            <span class="label">You Receive</span>
                            <span class="value" id="youReceive">0 BSV</span>
                        </div>
                    </div>
                    <p style="font-size:0.7rem;color:var(--text-dim);margin-bottom:12px;">
                        Buyer pays the listing price. Fees are deducted from your proceeds.
                    </p>
                    <div class="form-actions">
                        <button class="btn btn-primary" onclick="createListing('${origin}')" style="flex:1;">
                            List for Sale
                        </button>
                    </div>
                </div>
            `;
        } else if (isOwned && isListed) {
            return `
                <div class="listing-form">
                    <div class="form-title">✓ Currently Listed</div>
                    <div class="fee-breakdown">
                        <div class="fee-row">
                            <span class="label">Listing Price</span>
                            <span class="value">${satsToBsv(listing.fees.total_price)} BSV</span>
                        </div>
                        <div class="fee-row receives">
                            <span class="label">You Receive</span>
                            <span class="value">${satsToBsv(listing.fees.seller_receives)} BSV</span>
                        </div>
                    </div>
                    <button class="btn btn-secondary" onclick="cancelListing('${listing.id}')">
                        Cancel Listing
                    </button>
                </div>
            `;
        } else if (isListed && state.walletConnected) {
            return `
                <div class="listing-form">
                    <div class="form-title">💰 Purchase</div>
                    <div class="fee-breakdown">
                        <div class="fee-row total">
                            <span class="label">Price</span>
                            <span class="value">${satsToBsv(listing.fees.total_price)} BSV</span>
                        </div>
                    </div>
                    <button class="btn btn-primary" onclick="purchaseOrdinal('${listing.id}')">
                        Buy Now
                    </button>
                </div>
            `;
        } else if (isListed) {
            return `
                <div class="listing-form">
                    <div class="form-title">💰 For Sale</div>
                    <div class="fee-breakdown">
                        <div class="fee-row total">
                            <span class="label">Price</span>
                            <span class="value">${satsToBsv(listing.fees.total_price)} BSV</span>
                        </div>
                    </div>
                    <button class="btn btn-wallet" onclick="connectWallet()">
                        Connect to Buy
                    </button>
                </div>
            `;
        }
        
        return '';
    }

    // =================================================================
    // Listings Management
    // =================================================================

    async function loadMarketplaceListings() {
        const container = document.getElementById('marketplaceListings');
        
        try {
            const response = await fetch(`${CONFIG.API_BASE}/listings`);
            const data = await response.json();
            
            if (data.success) {
                state.marketplaceListings = data.listings;
                renderMarketplace();
            } else {
                throw new Error(data.message || 'Failed to load');
            }
        } catch (error) {
            console.error('Load listings error:', error);
            container.innerHTML = `
                <div class="empty-state">
                    <div class="empty-icon">🏪</div>
                    <h3 class="empty-title">Marketplace Offline</h3>
                    <p>Could not load listings</p>
                    <button class="btn btn-secondary" style="margin-top:12px;" onclick="loadMarketplaceListings()">
                        Retry
                    </button>
                </div>
            `;
        }
    }

    function renderMarketplace() {
        const container = document.getElementById('marketplaceListings');
        
        if (!state.marketplaceListings.length) {
            container.innerHTML = `
                <div class="empty-state">
                    <div class="empty-icon">🏪</div>
                    <h3 class="empty-title">No Listings</h3>
                    <p>Be the first to list an ordinal!</p>
                </div>
            `;
            return;
        }

        let html = '<div class="ordinals-grid">';
        
        state.marketplaceListings.forEach(listing => {
            const imageUrl = `${CONFIG.GORILLA_API}/files/inscriptions/${listing.origin}`;
            
            html += `
                <div class="ordinal-card listed" onclick="showListingDetails('${listing.id}')">
                    <div class="ordinal-preview">
                        <img src="${imageUrl}" 
                             loading="eager" 
                             crossorigin="anonymous" 
                             onload="this.classList.add('loaded')"
                             onerror="this.style.display='none';this.nextElementSibling.style.display='flex';">
                        <div class="placeholder" style="display:none;">◇</div>
                        <span class="badge badge-listed">For Sale</span>
                    </div>
                    <div class="ordinal-info">
                        <div class="ordinal-name">Listing</div>
                        <div class="ordinal-origin">${truncateOrigin(listing.origin)}</div>
                        <div class="ordinal-price">
                            <div class="price-value">${satsToBsv(listing.fees.total_price)} BSV</div>
                            <div class="price-label">Price</div>
                        </div>
                    </div>
                </div>
            `;
        });
        
        html += '</div>';
        container.innerHTML = html;
    }

    async function showListingDetails(listingId) {
        const listing = state.marketplaceListings.find(l => l.id === listingId);
        if (!listing) {
            showToast('Listing not found', 'error');
            return;
        }
        
        const ordinal = [...state.currentOrdinals, ...state.myOrdinals].find(o => o.origin === listing.origin);
        
        if (ordinal) {
            showOrdinalDetails(listing.origin, ordinal.owner_address === state.walletOrdAddress);
        } else {
            const imageUrl = `${CONFIG.GORILLA_API}/files/inscriptions/${listing.origin}`;
            
            document.getElementById('modalTitle').textContent = 'Listing';
            document.getElementById('modalContent').innerHTML = `
                <div class="modal-preview">
                    <img src="${imageUrl}" crossorigin="anonymous" 
                         onerror="this.outerHTML='<div class=\\'placeholder\\' style=\\'font-size:3rem;padding:40px;\\'>◇</div>'">
                </div>
                <div class="detail-grid">
                    <div class="detail-item full-width">
                        <div class="detail-label">Origin</div>
                        <div class="detail-value" style="font-size:0.7rem;">${listing.origin}</div>
                    </div>
                    <div class="detail-item">
                        <div class="detail-label">Seller</div>
                        <div class="detail-value">${truncateAddress(listing.seller_ord_address)}</div>
                    </div>
                    <div class="detail-item">
                        <div class="detail-label">Price</div>
                        <div class="detail-value highlight">${satsToBsv(listing.fees.total_price)} BSV</div>
                    </div>
                </div>
                ${state.walletConnected ? `
                    <div class="listing-form">
                        <button class="btn btn-primary" onclick="purchaseOrdinal('${listing.id}')" style="width:100%;">
                            Buy Now
                        </button>
                    </div>
                ` : `
                    <div class="listing-form">
                        <button class="btn btn-wallet" onclick="connectWallet()" style="width:100%;">
                            Connect to Buy
                        </button>
                    </div>
                `}
            `;
            
            document.getElementById('modalOverlay').classList.add('active');
        }
    }

    async function createListing(origin) {
        const priceInput = document.getElementById('listingPrice');
        const listingPriceBsv = parseFloat(priceInput?.value);
        
        if (!listingPriceBsv || listingPriceBsv <= 0) {
            showToast('Enter a valid price', 'error');
            return;
        }

        const ordinal = state.myOrdinals.find(o => o.origin === origin);
        if (!ordinal) {
            showToast('Ordinal not found', 'error');
            return;
        }

        const listingPriceSats = bsvToSats(listingPriceBsv);
        const fees = calculateFees(listingPriceSats, state.selectedTipPercent);

        if (fees.sellerReceives <= 0) {
            showToast('Price too low after fees', 'error');
            return;
        }

        try {
            showToast('Creating listing...', 'info');
            
            const response = await fetch(`${CONFIG.API_BASE}/listings`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({
                    origin: origin,
                    ordinal_utxo: {
                        txid: ordinal.txid,
                        vout: ordinal.vout,
                        satoshis: ordinal.satoshis || 1,
                        script: ''
                    },
                    seller_wants_satoshis: fees.sellerReceives,
                    tip_percent: state.selectedTipPercent,
                    seller_address: state.walletAddress,
                    seller_ord_address: state.walletOrdAddress,
                    listing_price_satoshis: listingPriceSats,
                    marketplace_fee_satoshis: fees.marketplaceFee,
                    tip_satoshis: fees.tip,
                    marketplace_fee_address: CONFIG.MARKETPLACE_FEE_ADDRESS
                })
            });

            const data = await response.json();
            
            if (data.success) {
                state.marketplaceListings.push(data.listing);
                showToast('Listed successfully!', 'success');
                closeModal();
                renderMyOrdinals();
                renderMarketplace();
            } else {
                throw new Error(data.message || 'Failed to create listing');
            }
        } catch (error) {
            console.error('Create listing error:', error);
            showToast('Error: ' + error.message, 'error');
        }
    }

    async function cancelListing(listingId) {
        try {
            showToast('Cancelling listing...', 'info');
            
            const response = await fetch(`${CONFIG.API_BASE}/listings/${listingId}/cancel`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({
                    listing_id: listingId,
                    seller_ord_address: state.walletOrdAddress
                })
            });

            const data = await response.json();
            
            if (data.success) {
                state.marketplaceListings = state.marketplaceListings.filter(l => l.id !== listingId);
                showToast('Listing cancelled', 'info');
                closeModal();
                renderMyOrdinals();
                renderMarketplace();
            } else {
                throw new Error(data.message || 'Failed to cancel');
            }
        } catch (error) {
            console.error('Cancel listing error:', error);
            showToast('Error: ' + error.message, 'error');
        }
    }

    async function purchaseOrdinal(listingId) {
        const listing = state.marketplaceListings.find(l => l.id === listingId);
        
        if (!listing) {
            showToast('Listing not found', 'error');
            return;
        }

        if (!state.walletConnected) {
            showToast('Connect wallet first', 'error');
            return;
        }

        if (listing.seller_ord_address === state.walletOrdAddress) {
            showToast('Cannot buy your own listing', 'error');
            return;
        }

        try {
            showToast('Preparing purchase...', 'info');
            
            // Try Yours Wallet's built-in purchaseOrdinal if available
            if (window.yours?.purchaseOrdinal) {
                const result = await window.yours.purchaseOrdinal({
                    outpoint: listing.origin,
                    marketplaceRate: CONFIG.MARKETPLACE_FEE_PERCENT / 100,
                    marketplaceAddress: CONFIG.MARKETPLACE_FEE_ADDRESS
                });
                
                if (result?.txid) {
                    showToast(`Purchase successful! Tx: ${result.txid.slice(0, 8)}...`, 'success');
                    
                    await fetch(`${CONFIG.API_BASE}/listings/${listingId}/sold`, {
                        method: 'POST',
                        headers: { 'Content-Type': 'application/json' },
                        body: JSON.stringify({
                            buyer_address: state.walletOrdAddress,
                            txid: result.txid
                        })
                    }).catch(console.error);
                    
                    state.marketplaceListings = state.marketplaceListings.filter(l => l.id !== listingId);
                    closeModal();
                    renderMarketplace();
                    return;
                }
            }
            
            showToast('Purchase requires Ordinal Lock setup. Coming soon!', 'info');
            
        } catch (error) {
            console.error('Purchase error:', error);
            showToast('Error: ' + error.message, 'error');
        }
    }

    // =================================================================
    // Health Check
    // =================================================================

    async function checkHealth() {
        try {
            const response = await fetch(`${CONFIG.API_BASE}/health`);
            const data = await response.json();
            
            document.getElementById('healthDot').classList.remove('error');
            document.getElementById('healthStatus').textContent = 'API Online';
            document.getElementById('listingsCount').textContent = `Listings: ${data.listings_count || 0}`;
            
        } catch (error) {
            document.getElementById('healthDot').classList.add('error');
            document.getElementById('healthStatus').textContent = 'API Offline';
            console.error('Health check failed:', error);
        }
    }

    // =================================================================
    // Initialize App
    // =================================================================

    function init() {
        console.log('🚀 4chain Ordinals Marketplace initializing...');
        
        // Health check
        checkHealth();
        setInterval(checkHealth, CONFIG.HEALTH_CHECK_INTERVAL);
        
        // Check for wallet
        checkYoursWallet();
        
        // Load marketplace
        loadMarketplaceListings();
        
        // Setup search enter key
        const addressInput = document.getElementById('addressInput');
        if (addressInput) {
            addressInput.addEventListener('keypress', (e) => {
                if (e.key === 'Enter') searchWallet();
            });
        }
        
        // Setup escape key for modal
        document.addEventListener('keydown', (e) => {
            if (e.key === 'Escape') closeModal();
        });
        
        console.log('✅ 4chain Ordinals Marketplace ready!');
    }

    // =================================================================
    // Export to window for onclick handlers
    // =================================================================
    
    window.connectWallet = connectWallet;
    window.switchTab = switchTab;
    window.closeModal = closeModal;
    window.searchWallet = searchWallet;
    window.loadMyOrdinals = loadMyOrdinals;
    window.showOrdinalDetails = showOrdinalDetails;
    window.loadMarketplaceListings = loadMarketplaceListings;
    window.showListingDetails = showListingDetails;
    window.createListing = createListing;
    window.cancelListing = cancelListing;
    window.purchaseOrdinal = purchaseOrdinal;
    window.selectTip = selectTip;
    window.updateFeeBreakdown = updateFeeBreakdown;
    window.copyToClipboard = copyToClipboard;

    // Export namespace
    window.FourChain = {
        CONFIG,
        state,
        init
    };

    // Initialize when DOM is ready
    if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', init);
    } else {
        init();
    }

})();
