// src/services/tx_builder.rs

use crate::models::{Listing, BuyerUtxo};
use bitcoin::{
    Address, Amount, Network, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Txid, Witness,
    consensus::serialize,
};
use bitcoin::hex::DisplayHex;
use std::str::FromStr;

pub fn build_purchase_tx(
    listing: &Listing,
    buyer_ord_address: &str,
    buyer_payment_address: &str,
    buyer_utxos: Vec<BuyerUtxo>,
    marketplace_fee_address: &str,
) -> Result<crate::models::PreparePurchaseResponse, Box<dyn std::error::Error>> {
    let mut tx = Transaction {
        version: bitcoin::transaction::Version(1),
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: vec![],
        output: vec![],
    };

    // Input 0: Ordinal UTXO
    let ordinal_utxo = &listing.ordinal_utxo;
    let ordinal_txid = Txid::from_str(&ordinal_utxo.txid)?;
    tx.input.push(TxIn {
        previous_output: OutPoint { txid: ordinal_txid, vout: ordinal_utxo.vout },
        script_sig: ScriptBuf::new(),
        sequence: Sequence::MAX,
        witness: Witness::new(),
    });

    // Buyer payment inputs
    let mut total_input_sats: u64 = 1; // from ordinal
    for utxo in &buyer_utxos {
        let txid = Txid::from_str(&utxo.txid)?;
        tx.input.push(TxIn {
            previous_output: OutPoint { txid, vout: utxo.vout },
            script_sig: ScriptBuf::new(),
            sequence: Sequence::MAX,
            witness: Witness::new(),
        });
        total_input_sats += utxo.satoshis;
    }

    // Output 0: Ordinal to buyer (1 sat)
    let buyer_ord_addr = Address::from_str(buyer_ord_address)?.require_network(Network::Bitcoin)?;
    tx.output.push(TxOut {
        value: Amount::from_sat(1),
        script_pubkey: buyer_ord_addr.script_pubkey(),
    });

    // Output 1: Seller receives their full requested amount
    let seller_addr = Address::from_str(&listing.seller_address)?.require_network(Network::Bitcoin)?;
    tx.output.push(TxOut {
        value: Amount::from_sat(listing.fees.seller_receives),
        script_pubkey: seller_addr.script_pubkey(),
    });

    // Output 2: Marketplace receives 1% fee + tip (donation)
    let total_marketplace_sats = listing.fees.marketplace_fee + listing.fees.tip_amount;
    if total_marketplace_sats > 0 {
        let marketplace_addr = Address::from_str(marketplace_fee_address)?.require_network(Network::Bitcoin)?;
        tx.output.push(TxOut {
            value: Amount::from_sat(total_marketplace_sats),
            script_pubkey: marketplace_addr.script_pubkey(),
        });
    }

    // Change output to buyer
    let total_fixed_outputs = 1 + listing.fees.seller_receives + total_marketplace_sats;
    let estimated_fee = 300u64; // conservative miner fee
    let change = total_input_sats.saturating_sub(total_fixed_outputs + estimated_fee);

    if change >= 546 { // dust threshold
        let change_addr = Address::from_str(buyer_payment_address)?.require_network(Network::Bitcoin)?;
        tx.output.push(TxOut {
            value: Amount::from_sat(change),
            script_pubkey: change_addr.script_pubkey(),
        });
    }

    // Sig requests for buyer inputs only (skip ordinal input)
    let mut sig_requests = Vec::new();
    for (i, utxo) in buyer_utxos.iter().enumerate() {
        let input_index = i + 1; // input 0 is ordinal
        sig_requests.push(crate::models::SigRequest {
            input_index: input_index as u32,
            prev_txid: utxo.txid.clone(),
            prev_vout: utxo.vout,
            satoshis: utxo.satoshis,
            script_hex: utxo.script_hex.clone(),
        });
    }

    let raw_bytes = serialize(&tx);
    let raw_tx_hex = raw_bytes.as_hex().to_string();

    Ok(crate::models::PreparePurchaseResponse {
        raw_tx_hex,
        sig_requests,
    })
}