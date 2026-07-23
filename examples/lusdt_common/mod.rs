use std::env;
use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use kaleidoswap_sdk::boltz::BoltzApiClientV2;
use kaleidoswap_sdk::error::Error as SdkError;
use kaleidoswap_sdk::network::{LiquidChain, Network};
use kaleidoswap_sdk::util::secrets::SwapMasterKey;
use kaleidoswap_sdk::Keypair;

pub fn sdk<T>(result: std::result::Result<T, SdkError>, operation: &str) -> Result<T> {
    result.map_err(|error| anyhow!("{operation}: {error}"))
}

pub fn required_env(name: &str) -> Result<String> {
    env::var(name).with_context(|| format!("{name} must be set"))
}

pub fn optional_env(name: &str, default: &str) -> String {
    env::var(name).unwrap_or_else(|_| default.to_owned())
}

pub fn optional_u64(name: &str, default: u64) -> Result<u64> {
    match env::var(name) {
        Ok(value) => value
            .parse()
            .with_context(|| format!("{name} must be an unsigned integer")),
        Err(env::VarError::NotPresent) => Ok(default),
        Err(error) => Err(error).with_context(|| format!("read {name}")),
    }
}

pub fn network() -> Result<(Network, LiquidChain)> {
    match optional_env("KALEIDO_NETWORK", "regtest")
        .to_ascii_lowercase()
        .as_str()
    {
        "mainnet" => Ok((Network::Mainnet, LiquidChain::Liquid)),
        "testnet" => Ok((Network::Testnet, LiquidChain::LiquidTestnet)),
        "regtest" => Ok((Network::Regtest, LiquidChain::LiquidRegtest)),
        other => bail!("unsupported KALEIDO_NETWORK '{other}'; use mainnet, testnet, or regtest"),
    }
}

pub fn api_client() -> BoltzApiClientV2 {
    let url = optional_env("KALEIDO_MAKER_URL", "http://localhost:9001/v2");
    BoltzApiClientV2::new(
        url.trim_end_matches('/').to_owned(),
        Some(Duration::from_secs(30)),
    )
}

pub fn swap_key(network: Network) -> Result<(u64, Keypair)> {
    let mnemonic = required_env("KALEIDO_SWAP_MNEMONIC")?;
    let index = optional_u64("KALEIDO_SWAP_INDEX", 0)?;
    let master = sdk(
        SwapMasterKey::from_mnemonic(&mnemonic, None, network),
        "load swap mnemonic",
    )?;
    let key = sdk(master.derive_swapkey(index), "derive per-swap key")?;
    Ok((index, key))
}

pub async fn wait_for_status(api: &BoltzApiClientV2, swap_id: &str, target: &str) -> Result<()> {
    let timeout = Duration::from_secs(optional_u64("KALEIDO_WAIT_TIMEOUT_SECS", 3_600)?);
    let deadline = Instant::now() + timeout;
    let mut previous = String::new();

    loop {
        let response = sdk(api.get_swap(swap_id).await, "query swap status")?;
        if response.status != previous {
            println!("status: {}", response.status);
            previous.clone_from(&response.status);
        }
        if response.status == target {
            return Ok(());
        }
        if is_failure_status(&response.status) {
            bail!("swap entered terminal failure status {}", response.status);
        }
        if Instant::now() >= deadline {
            bail!("timed out waiting for {target}; last status was {previous}");
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

#[allow(dead_code)] // Shared support is compiled independently for each example.
pub async fn wait_for_file(path: &Path) -> Result<()> {
    let timeout = Duration::from_secs(optional_u64("KALEIDO_WAIT_TIMEOUT_SECS", 3_600)?);
    let deadline = Instant::now() + timeout;
    while !path.is_file() {
        if Instant::now() >= deadline {
            bail!("timed out waiting for wallet response {}", path.display());
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
    Ok(())
}

fn is_failure_status(status: &str) -> bool {
    matches!(
        status,
        "swap.expired"
            | "swap.failed"
            | "transaction.lockupFailed"
            | "transaction.failed"
            | "invoice.failedToPay"
    )
}
