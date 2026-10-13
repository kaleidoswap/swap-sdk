use crate::boltz::Error;

/// Resolve a Lightning address or encoded LNURL-pay request to a fixed-amount
/// invoice. Whole satoshis; no payment or swap creation occurs.
#[uniffi::export(async_runtime = "tokio")]
pub async fn resolve_lightning_address(address: String, amount_sat: u64) -> Result<String, Error> {
    Ok(kaleidorg_swap_sdk::util::lnurl::resolve_lightning_address(&address, amount_sat).await?)
}
