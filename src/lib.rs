//! A boltz client for submarine, reverse and chain swaps between Bitcoin, Lightning & Liquid
//! Refer to tests/ folder for usage

/// Error Module
pub mod error;
/// Blockchain Network module. Currently only contains electrum interface.
pub mod network;
/// core swap logic
pub mod swaps;
/// utilities (key, preimage, error)
pub mod util;

// Re-export common libs, so callers can make use of them and avoid version conflicts
pub use bitcoin;
pub use elements;
pub use lightning_invoice;
#[cfg(feature = "lnurl")]
pub use lnurl;
pub use reqwest;

// Re-export relevant structs under kaleidorg_swap_sdk::StructName for simplicity
pub use bitcoin::{
    blockdata::locktime::absolute::LockTime,
    hashes::hash160::Hash,
    secp256k1::{Keypair, Secp256k1},
    Address, Amount, PublicKey,
};
pub use elements::{
    address::Address as ElementsAddress,
    hex::ToHex,
    locktime::LockTime as ElementsLockTime,
    pset::serialize::Serialize,
    secp256k1_zkp::{Keypair as ZKKeyPair, Secp256k1 as ZKSecp256k1},
};
pub use lightning_invoice::Bolt11Invoice;

#[allow(deprecated)]
pub use swaps::liquid::{LBtcSwapScript, LBtcSwapTx};
pub use swaps::{
    bitcoin::{BtcSwapScript, BtcSwapTx},
    boltz, kaleido,
    liquid::{decode_swap_output, LiquidAssetContext, LiquidSwapScript, LiquidSwapTx},
};
pub use util::fees;
