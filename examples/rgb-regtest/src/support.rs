use anyhow::{anyhow, bail, ensure, Context, Result};
use bitcoin::{consensus, Transaction};
use ldk_server_client::{client::LdkServerClient, ldk_server_grpc::api::*};
use rgb_lib::{
    wallet::{
        DatabaseType, Online, OnlineOptions, RgbWalletOpsOffline, RgbWalletOpsOnline,
        SinglesigKeys, WalletData,
    },
    AssetSchema, BitcoinNetwork, Wallet,
};
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

pub const ESPLORA: &str = "http://127.0.0.1:23002";
pub const PROXY: &str = "rpc://127.0.0.1:23000/json-rpc";
pub const MAKER: &str = "http://127.0.0.1:29420/v2";
pub const ISSUED: u64 = 2_000_000_000;
pub const INVENTORY: u64 = 1_000_000_000;

pub fn sdk<T>(result: std::result::Result<T, kaleidorg_swap_sdk::error::Error>) -> Result<T> {
    result.map_err(|e| anyhow!("SDK: {e}"))
}
pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}
pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("run")
}
pub fn script(args: &[&str]) -> Result<String> {
    let out = Command::new(Path::new(env!("CARGO_MANIFEST_DIR")).join("regtest.sh"))
        .args(args)
        .output()?;
    ensure!(
        out.status.success(),
        "regtest.sh {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    Ok(String::from_utf8(out.stdout)?.trim().to_owned())
}
pub fn private_json(path: &Path, value: &impl serde::Serialize) -> Result<()> {
    fs::create_dir_all(path.parent().context("state parent")?)?;
    let temporary = path.with_extension("tmp");
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    use std::io::Write;
    let mut file = options.open(&temporary)?;
    file.write_all(&serde_json::to_vec_pretty(value)?)?;
    file.sync_all()?;
    fs::rename(temporary, path)?;
    fs::File::open(path.parent().context("state parent")?)?.sync_all()?;
    Ok(())
}
pub fn open_wallet(dir: &Path, mnemonic: &str) -> Result<Wallet> {
    fs::create_dir_all(dir)?;
    let keys = rgb_lib::restore_keys(
        BitcoinNetwork::Regtest,
        mnemonic.to_owned(),
        Default::default(),
    )?;
    Ok(Wallet::new(
        WalletData {
            data_dir: dir.to_str().context("wallet directory")?.into(),
            bitcoin_network: BitcoinNetwork::Regtest,
            database_type: DatabaseType::Sqlite,
            max_allocations_per_utxo: 1,
            supported_schemas: vec![AssetSchema::Nia, AssetSchema::Cfa, AssetSchema::Ifa],
            reuse_addresses: false,
        },
        SinglesigKeys::from_keys(&keys, None),
    )?)
}
pub fn online(wallet: &mut Wallet) -> Result<Online> {
    Ok(wallet.go_online(OnlineOptions {
        indexer_url: ESPLORA.into(),
        skip_consistency_check: false,
        vanilla_sync_lookback: 20,
    })?)
}
pub async fn get(path: &str) -> Result<Value> {
    let response = reqwest::get(format!("{MAKER}{path}")).await?;
    let status = response.status();
    let body = response.text().await?;
    ensure!(status.is_success(), "GET {path}: {status}: {body}");
    Ok(serde_json::from_str(&body)?)
}
pub async fn rpc(method: &str, params: Value) -> Result<Value> {
    let response: Value = reqwest::Client::new()
        .post("http://127.0.0.1:23443/wallet/miner")
        .basic_auth("user", Some("pass"))
        .json(&json!({"jsonrpc":"1.0", "id":"rgb-sdk", "method":method, "params":params}))
        .send()
        .await?
        .json()
        .await?;
    ensure!(
        response["error"].is_null(),
        "bitcoin {method}: {}",
        response["error"]
    );
    Ok(response["result"].clone())
}
pub async fn tip() -> Result<u32> {
    Ok(reqwest::get(format!("{ESPLORA}/blocks/tip/height"))
        .await?
        .error_for_status()?
        .text()
        .await?
        .trim()
        .parse()?)
}
pub async fn mine(blocks: u32) -> Result<()> {
    let target = rpc("getblockcount", json!([]))
        .await?
        .as_u64()
        .context("chain height")?
        + u64::from(blocks);
    script(&["mine", &blocks.to_string()])?;
    let deadline = Instant::now() + Duration::from_secs(60);
    while u64::from(tip().await.unwrap_or(0)) < target {
        ensure!(
            Instant::now() < deadline,
            "indexer did not reach block {target}"
        );
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    Ok(())
}
pub async fn transaction(txid: &str) -> Result<Transaction> {
    let hex = rpc("getrawtransaction", json!([txid])).await?;
    Ok(consensus::deserialize(&hex::decode(
        hex.as_str().context("transaction hex")?,
    )?)?)
}
pub async fn wait_status(id: &str, target: &str, mine_periodically: bool) -> Result<Value> {
    let deadline = Instant::now() + Duration::from_secs(180);
    let mut last = String::new();
    let mut count = 0;
    loop {
        let state = get(&format!("/swap/{id}")).await?;
        let status = state["status"].as_str().unwrap_or("");
        if status != last {
            println!("swap {id}: {status}");
            last = status.into();
        }
        if status == target {
            return Ok(state);
        }
        ensure!(
            !matches!(
                status,
                "transaction.failed" | "swap.expired" | "transaction.refunded"
            ),
            "unexpected status {status}"
        );
        ensure!(
            Instant::now() < deadline,
            "waiting for {target}: last={last}"
        );
        if mine_periodically && count % 5 == 0 {
            mine(1).await?;
        }
        count += 1;
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}
pub async fn wait_balance(
    wallet: &mut Wallet,
    online: Online,
    asset: &str,
    expected: u64,
) -> Result<()> {
    for _ in 0..60 {
        let actual = tokio::task::block_in_place(|| -> Result<u64> {
            wallet.refresh(online, None, Vec::new(), false)?;
            Ok(wallet.get_asset_balance(asset.into())?.settled)
        })?;
        if actual == expected {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    bail!("RGB wallet balance did not settle to {expected}")
}
pub async fn node(name: &str) -> Result<LdkServerClient> {
    let port = if name == "maker" { 23636 } else { 23646 };
    let key = fs::read(root().join(format!("ldk-{name}/regtest/api_key")))?;
    let cert = fs::read(root().join(format!("certs/{name}.crt")))?;
    LdkServerClient::new(format!("localhost:{port}"), hex::encode(key), &cert)
        .map_err(|e| anyhow!("LDK {name}: {e}"))
}
pub async fn provision_lightning() -> Result<()> {
    let maker = node("maker").await?;
    let taker = node("taker").await?;
    let peer = maker.get_node_info(GetNodeInfoRequest {}).await?;
    taker
        .connect_peer(ConnectPeerRequest {
            node_pubkey: peer.node_id.clone(),
            address: "ldk-maker:9735".into(),
            persist: true,
        })
        .await?;
    for client in [&maker, &taker] {
        let address = client
            .onchain_receive(OnchainReceiveRequest {})
            .await?
            .address;
        script(&["sendtoaddress", &address, "0.1"])?;
    }
    mine(6).await?;
    for client in [&maker, &taker] {
        let deadline = Instant::now() + Duration::from_secs(60);
        while client
            .get_balances(GetBalancesRequest {})
            .await?
            .spendable_onchain_balance_sats
            < 9_000_000
        {
            ensure!(Instant::now() < deadline, "LDK did not sync its funding");
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }
    taker
        .open_channel(OpenChannelRequest {
            node_pubkey: peer.node_id,
            address: "ldk-maker:9735".into(),
            channel_amount_sats: 4_000_000,
            push_to_counterparty_msat: Some(2_000_000_000),
            channel_config: None,
            announce_channel: false,
            disable_counterparty_reserve: false,
        })
        .await?;
    tokio::time::sleep(Duration::from_secs(3)).await;
    mine(12).await?;
    for client in [&maker, &taker] {
        let deadline = Instant::now() + Duration::from_secs(90);
        while !client
            .list_channels(ListChannelsRequest {})
            .await?
            .channels
            .iter()
            .any(|c| c.is_usable)
        {
            ensure!(
                Instant::now() < deadline,
                "LDK channel did not become usable"
            );
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }
    println!("real Lightning channel ready with liquidity in both directions");
    Ok(())
}

pub struct MakerProcess(Child);
impl MakerProcess {
    pub async fn start(identity: &Value) -> Result<Self> {
        let executable = std::env::var("RGB_MAKER_BIN")
            .context("RGB_MAKER_BIN: pinned maker built with --features ldk-server")?;
        let log = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(root().join("maker.log"))?;
        let refund = rpc("getnewaddress", json!([])).await?;
        let key = hex::encode(fs::read(root().join("ldk-maker/regtest/api_key"))?);
        let child = Command::new(executable)
            .current_dir(root())
            .env("SENTRY_DSN", "")
            .env("RUST_LOG", "info,sqlx=warn")
            .env("MAKER_ADMIN__ALLOW_UNAUTHENTICATED", "true")
            .env("MAKER_SERVER__LISTEN", "127.0.0.1:29420")
            .env(
                "MAKER_DATABASE__URL",
                "postgres://postgres:postgres@127.0.0.1:25433/maker",
            )
            .env("MAKER_LDK_SERVER__ADDRESS", "localhost:23636")
            .env("MAKER_LDK_SERVER__API_KEY_HEX", key)
            .env(
                "MAKER_LDK_SERVER__TLS_CERT_PATH",
                root().join("certs/maker.crt"),
            )
            .env(
                "MAKER_SWAP__MASTER_SEED_HEX",
                "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff",
            )
            .env("MAKER_SWAP__CONFIRMATION_TARGET", "1")
            .env("MAKER_ESPLORA__URL", ESPLORA)
            .env("MAKER_ESPLORA__POLL_INTERVAL_SECS", "1")
            .env(
                "MAKER_REFUND__DESTINATION_ADDRESS",
                refund.as_str().context("refund address")?,
            )
            .env("MAKER_PRICEFEED__SOURCE", "remote")
            .env("MAKER_PRICEFEED__REMOTE_URL", "http://127.0.0.1:29421")
            .env("MAKER_PRICEFEED__ALLOW_INSECURE_HTTP", "true")
            .env("MAKER_RGB__ENABLED", "true")
            .env("MAKER_RGB__NETWORK", "regtest")
            .env("MAKER_RGB__DATA_DIR", root().join("rgb-maker"))
            .env(
                "MAKER_RGB__MNEMONIC",
                identity["makerMnemonic"]
                    .as_str()
                    .context("maker mnemonic")?,
            )
            .env("MAKER_RGB__INDEXER_URL", ESPLORA)
            .env("MAKER_RGB__PROXY_URL", PROXY)
            .env(
                "MAKER_RGB__SUPPORTED_ASSETS",
                identity["assetId"].as_str().context("asset pin")?,
            )
            .env("MAKER_RGB__MIN_CONFIRMATIONS", "1")
            .env("MAKER_RGB__SYNC_INTERVAL_SECS", "1")
            .env("MAKER_RGB__HTLC_POLL_INTERVAL_SECS", "1")
            .env("MAKER_RGB__HTLC_CLAIM_FEE_RATE", "5")
            .env("MAKER_RGB__HTLC_TAKER_CLAIM_FEE_RATE", "5")
            .stdin(Stdio::null())
            .stdout(Stdio::from(log.try_clone()?))
            .stderr(Stdio::from(log))
            .spawn()?;
        let mut process = Self(child);
        for _ in 0..90 {
            ensure!(
                process.0.try_wait()?.is_none(),
                "maker exited; inspect run/maker.log"
            );
            if get("/health").await.is_ok() {
                return Ok(process);
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        bail!("maker did not start; inspect run/maker.log")
    }
}
impl Drop for MakerProcess {
    fn drop(&mut self) {
        let _ = Command::new("kill")
            .args(["-TERM", &self.0.id().to_string()])
            .status();
        let _ = self.0.wait();
    }
}
