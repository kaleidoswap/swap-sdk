//! Native, opt-in end-to-end exercise. All funds are on the private regtest stack.
mod amount_audit;
mod bridge;
mod pricefeed;
mod support;
mod wallet;
use anyhow::{bail, ensure, Context, Result};
use bitcoin::{OutPoint, PublicKey, TxOut};
use kaleidorg_swap_sdk::{
    network::BitcoinChain,
    swaps::{
        boltz::{
            BoltzApiClientV2, CreateReverseRequest, CreateSubmarineRequest,
            CreateSubmarineResponse, SwapTxKind,
        },
        rgb::{PreparedRgbSpend, RgbSpendFunding},
    },
    util::secrets::Preimage,
    BtcSwapScript, Keypair, Secp256k1,
};
use ldk_server_client::ldk_server_grpc::api::*;
use rgb_lib::{
    wallet::{rust_only::ExpectedTransfer, Online, Recipient, RgbWalletOpsOffline, WitnessData},
    AssetSchema, Assignment, BitcoinNetwork, Wallet,
};
use serde_json::{json, Value};
use std::{fs, str::FromStr, time::Duration};
use support::*;
macro_rules! rgb {
    ($expression:expr) => {
        tokio::task::block_in_place(|| $expression)
    };
}
const CHAIN: BitcoinChain = BitcoinChain::BitcoinRegtest;
fn key() -> Keypair {
    Keypair::new(
        &Secp256k1::new(),
        &mut bitcoin::secp256k1::rand::thread_rng(),
    )
}
fn identity() -> Result<Value> {
    Ok(serde_json::from_slice(&fs::read(
        root().join("identity.json"),
    )?)?)
}
fn recover(
    tag: &str,
    key: &Keypair,
    preimage: Option<&Preimage>,
    response: &impl serde::Serialize,
) -> Result<()> {
    private_json(
        &root().join(format!("{tag}.json")),
        &json!({"secretKey":hex::encode(key.secret_bytes()),"preimage":preimage.and_then(|p|p.bytes).map(hex::encode),"response":response}),
    )
}
async fn bootstrap() -> Result<()> {
    ensure!(
        !root().join("identity.json").exists(),
        "existing run: preserve recovery state; use a fresh private stack to bootstrap again"
    );
    let maker_mnemonic =
        rgb_lib::generate_keys(BitcoinNetwork::Regtest, Default::default()).mnemonic;
    let taker_mnemonic =
        rgb_lib::generate_keys(BitcoinNetwork::Regtest, Default::default()).mnemonic;
    let reverse_mnemonic =
        rgb_lib::generate_keys(BitcoinNetwork::Regtest, Default::default()).mnemonic;
    let mut id = json!({"makerMnemonic":maker_mnemonic,"takerMnemonic":taker_mnemonic,"reverseMnemonic":reverse_mnemonic});
    private_json(&root().join("identity.json"), &id)?;
    let mut maker = rgb!(open_wallet(&root().join("rgb-maker"), &maker_mnemonic))?;
    let mut taker = rgb!(open_wallet(&root().join("rgb-taker"), &taker_mnemonic))?;
    for w in [&mut maker, &mut taker] {
        let address = rgb!(w.get_address())?;
        script(&["sendtoaddress", &address, "0.01"])?;
    }
    mine(1).await?;
    let mo = rgb!(online(&mut maker))?;
    let to = rgb!(online(&mut taker))?;
    for (w, o) in [(&mut maker, mo), (&mut taker, to)] {
        rgb!(w.create_utxos(o, false, Some(4), Some(5000), 2, false))?;
    }
    mine(1).await?;
    let asset = rgb!(taker.issue_asset_nia("USDT".into(), "Regtest USDT".into(), 6, vec![ISSUED]))?
        .asset_id;
    id["assetId"] = json!(asset);
    private_json(&root().join("identity.json"), &id)?;
    let receive =
        rgb!(maker.witness_receive(None, Assignment::Any, now() + 3600, vec![PROXY.into()], 1))?;
    rgb!(wallet::send(
        &mut taker,
        to,
        &asset,
        Recipient {
            recipient_id: receive.recipient_id,
            assignment: Assignment::Fungible(INVENTORY),
            witness_data: Some(WitnessData {
                amount_sat: 1000,
                blinding: None
            }),
            transport_endpoints: vec![PROXY.into()]
        }
    ))?;
    mine(1).await?;
    wait_balance(&mut maker, mo, &asset, INVENTORY).await?;
    wait_balance(&mut taker, to, &asset, ISSUED - INVENTORY).await?;
    drop(maker);
    drop(taker);
    provision_lightning().await?;
    private_json(
        &root().join("bootstrap-complete.json"),
        &json!({"assetId":asset}),
    )?;
    println!("RGB asset issued and maker inventory settled");
    Ok(())
}
async fn outpoint(address: &str) -> Result<(OutPoint, TxOut)> {
    for _ in 0..60 {
        let unspents: Value = reqwest::get(format!("{ESPLORA}/address/{address}/utxo"))
            .await?
            .error_for_status()?
            .json()
            .await?;
        if let Some(u) = unspents.as_array().context("UTXOs")?.first() {
            let op = OutPoint::new(
                bitcoin::Txid::from_str(u["txid"].as_str().context("txid")?)?,
                u["vout"].as_u64().context("vout")? as u32,
            );
            let tx = transaction(&op.txid.to_string()).await?;
            return Ok((op, tx.output[op.vout as usize].clone()));
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    bail!("lock output not found")
}
async fn submarine(
    api: &BoltzApiClientV2,
    wallet: &mut Wallet,
    online: Online,
    asset: &str,
    invoice: &str,
    tag: &str,
) -> Result<(CreateSubmarineResponse, Keypair, OutPoint, TxOut)> {
    let pairs = get("/swap/submarine").await?;
    let pair_hash = pairs["USDT-RGB"]["BTC"]["hash"]
        .as_str()
        .context("submarine pair hash")?
        .to_owned();
    let key = key();
    let pk = PublicKey::new(key.public_key());
    let response = sdk(api
        .post_swap_req(&CreateSubmarineRequest {
            from: "USDT-RGB".into(),
            to: "BTC".into(),
            invoice: invoice.into(),
            refund_public_key: pk,
            pair_hash: Some(pair_hash),
            referral_id: None,
            webhook: None,
        })
        .await)?;
    sdk(response.validate_rgb(invoice, &pk, CHAIN, asset))?;
    recover(tag, &key, None, &response)?;
    let lock = response.rgb.as_ref().context("RGB lock")?;
    let txid = rgb!(wallet::lock(wallet, online, lock))?;
    mine(1).await?;
    let tx = transaction(&txid).await?;
    let script = bitcoin::ScriptBuf::from_bytes(hex::decode(&lock.script_pubkey)?);
    let (vout, prevout) = tx
        .output
        .iter()
        .enumerate()
        .find(|(_, o)| o.script_pubkey == script)
        .context("validated lock output")?;
    let op = OutPoint::new(tx.compute_txid(), vout.try_into()?);
    ensure!(prevout.value.to_sat() == lock.htlc_sat, "lock sats");
    Ok((response, key, op, prevout.clone()))
}
async fn run() -> Result<()> {
    ensure!(
        root().join("bootstrap-complete.json").exists(),
        "run bootstrap first"
    );
    ensure!(!root().join("happy.json").exists(),"swap run already attempted; preserve recovery files and inspect wallet/maker logs before retrying");
    let id = identity()?;
    let asset = id["assetId"].as_str().context("asset")?;
    let feed = pricefeed::start().await?;
    let maker = MakerProcess::start(&id).await?;
    script(&["publish-pairs"])?;
    drop(maker);
    let maker = MakerProcess::start(&id).await?;
    tokio::time::sleep(Duration::from_secs(3)).await;
    let api = BoltzApiClientV2::new(MAKER.into(), Some(Duration::from_secs(30)));
    // Check discovery before either taker pays, including unrelated seeded routes.
    get("/swap/reverse").await?;
    let ln = node("taker").await?;
    let mut taker = rgb!(open_wallet(
        &root().join("rgb-taker"),
        id["takerMnemonic"].as_str().context("mnemonic")?
    ))?;
    let to = rgb!(online(&mut taker))?;
    let invoice = ln
        .bolt11_receive(Bolt11ReceiveRequest {
            amount_msat: Some(100_000_000),
            description: None,
            expiry_secs: 3600,
        })
        .await?;
    let (happy, _, happy_op, _) =
        submarine(&api, &mut taker, to, asset, &invoice.invoice, "happy").await?;
    wait_status(&happy.id, "transaction.claimed", true).await?;
    mine(1).await?;
    let remaining = ISSUED - INVENTORY - happy.expected_amount;
    wait_balance(&mut taker, to, asset, remaining).await?;
    let received = ln
        .get_payment_details(GetPaymentDetailsRequest {
            payment_id: invoice.payment_hash,
        })
        .await?
        .payment
        .context("received payment")?;
    ensure!(received.status == 1, "Lightning invoice did not succeed");
    println!("submarine succeeded: real LN payment and maker claim");

    let hold_preimage = Preimage::random();
    let hold_hash = hold_preimage.sha256.to_string();
    let hold = ln
        .bolt11_receive_for_hash(Bolt11ReceiveForHashRequest {
            amount_msat: Some(100_000_000),
            description: None,
            expiry_secs: 3600,
            payment_hash: hold_hash.clone(),
            min_final_cltv_expiry_delta: None,
        })
        .await?;
    let (refund, rkey, rop, rprev) =
        submarine(&api, &mut taker, to, asset, &hold.invoice, "refund").await?;
    wait_status(&refund.id, "invoice.pending", false).await?;
    tokio::time::sleep(Duration::from_secs(2)).await;
    ln.bolt11_fail_for_hash(Bolt11FailForHashRequest {
        payment_hash: hold_hash,
    })
    .await?;
    wait_status(&refund.id, "invoice.failedToPay", false).await?;
    let script = sdk(BtcSwapScript::submarine_from_swap_resp(
        &refund,
        PublicKey::new(rkey.public_key()),
    ))?;
    let destination = rgb!(wallet::destination(&mut taker))?;
    let insufficient = PreparedRgbSpend::new(
        SwapTxKind::Refund,
        script.clone(),
        &destination,
        CHAIN,
        (rop, rprev.clone()),
        RgbSpendFunding::HtlcValue { fee_rate_sat_vb: 5 },
        5000,
    );
    ensure!(
        matches!(
            insufficient,
            Err(kaleidorg_swap_sdk::error::Error::RgbFeeInputRequired)
        ),
        "insufficient HTLC fee must request BTC input"
    );
    let prepared = sdk(PreparedRgbSpend::new(
        SwapTxKind::Refund,
        script,
        &destination,
        CHAIN,
        (rop, rprev),
        RgbSpendFunding::CallerInputs,
        5000,
    ))?;
    let prepared = rgb!(wallet::fund(&mut taker, to, prepared))?;
    let (operation, colored) = rgb!(wallet::color(&mut taker, &prepared))?;
    let finalized = sdk(prepared.finalize_refund(colored, &rkey))?;
    let (psbt, tx) = rgb!(wallet::sign_wallet_inputs(&taker, finalized))?;
    let early = rpc(
        "testmempoolaccept",
        json!([[hex::encode(bitcoin::consensus::serialize(&tx))]]),
    )
    .await?;
    ensure!(
        early[0]["allowed"] == false && early[0]["reject-reason"] == "non-final",
        "early refund not refused for CLTV: {early}"
    );
    let timeout = u32::try_from(refund.timeout_block_height)?;
    let height = tip().await?;
    ensure!(height < timeout, "refund test needs unexpired HTLC");
    mine(timeout - height + 1).await?;
    wallet::broadcast_apply(&mut taker, to, &operation, asset, &psbt, &tx).await?;
    wait_balance(&mut taker, to, asset, remaining).await?;
    ensure!(tx.input[0].witness.len() == 3, "refund leaf witness");
    println!("submarine refund restored RGB allocation; early CLTV spend rejected");

    let mut receiver = rgb!(open_wallet(
        &root().join("rgb-reverse"),
        id["reverseMnemonic"].as_str().context("reverse mnemonic")?
    ))?;
    let ro = rgb!(online(&mut receiver))?;
    let balance = rgb!(receiver.get_btc_balance(Some(ro), false))?;
    ensure!(
        balance.vanilla.settled == 0 && balance.colored.settled == 0,
        "reverse wallet must have no BTC"
    );
    let preimage = Preimage::random();
    let ckey = key();
    let pk = PublicKey::new(ckey.public_key());
    let pairs = get("/swap/reverse").await?;
    let pair_hash = pairs["BTC"]["USDT-RGB"]["hash"]
        .as_str()
        .context("reverse pair hash")?
        .to_owned();
    let reverse = sdk(api
        .post_reverse_req(CreateReverseRequest {
            from: "BTC".into(),
            to: "USDT-RGB".into(),
            claim_public_key: pk,
            invoice: None,
            invoice_amount: Some(100000),
            preimage_hash: Some(preimage.sha256),
            description: None,
            description_hash: None,
            address: None,
            address_signature: None,
            referral_id: None,
            webhook: None,
            pair_hash: Some(pair_hash),
        })
        .await)?;
    sdk(reverse.validate_rgb(&preimage, &pk, CHAIN, asset))?;
    recover("reverse", &ckey, Some(&preimage), &reverse)?;
    let payment = ln
        .bolt11_send(Bolt11SendRequest {
            invoice: reverse.invoice.clone().context("reverse invoice")?,
            amount_msat: None,
            route_parameters: None,
        })
        .await?;
    wait_status(&reverse.id, "transaction.mempool", false).await?;
    let (cop, cprev) = outpoint(&reverse.lockup_address).await?;
    mine(1).await?;
    let lock = reverse.rgb.as_ref().context("reverse RGB")?;
    let blinding = lock.blinding.parse()?;
    let (consignment, assignments) = rgb!(receiver.fetch_and_accept_transfer_by_recipient_id(
        ro,
        lock.recipient_id.clone(),
        lock.recipient_id.clone(),
        &lock.transport_endpoints[0],
        blinding,
        lock.min_confirmations,
        ExpectedTransfer {
            asset_id: asset.into(),
            asset_schema: AssetSchema::Nia,
            assignment: Assignment::Fungible(lock.amount)
        }
    ))?;
    ensure!(
        assignments == vec![Assignment::Fungible(lock.amount)],
        "accepted allocation"
    );
    rgb!(receiver.save_new_asset(ro, consignment, cop.txid.to_string()))?;
    let destination = rgb!(wallet::destination(&mut receiver))?;
    let script = sdk(BtcSwapScript::reverse_from_swap_resp(&reverse, pk))?;
    let prepared = sdk(PreparedRgbSpend::new(
        SwapTxKind::Claim,
        script,
        &destination,
        CHAIN,
        (cop, cprev),
        RgbSpendFunding::HtlcValue {
            fee_rate_sat_vb: lock.claim_fee_rate.context("claim fee")?,
        },
        5000,
    ))?;
    let (operation, colored) = rgb!(wallet::color(&mut receiver, &prepared))?;
    let finalized = sdk(prepared.finalize_claim(colored, &ckey, &preimage))?;
    let psbt = finalized.psbt;
    let claim = finalized
        .transaction
        .context("self funded claim must be final")?;
    ensure!(
        claim.input.len() == 1 && claim.input[0].witness.len() == 4,
        "claim without wallet BTC inputs"
    );
    ensure!(
        claim.input[0].witness.nth(1) == preimage.bytes.as_ref().map(|p| p.as_slice()),
        "claim reveals correct preimage"
    );
    wallet::broadcast_apply(&mut receiver, ro, &operation, asset, &psbt, &claim).await?;
    wait_balance(&mut receiver, ro, asset, lock.amount).await?;
    wait_status(&reverse.id, "invoice.settled", false).await?;
    let paid = ln
        .get_payment_details(GetPaymentDetailsRequest {
            payment_id: payment.payment_id,
        })
        .await?
        .payment
        .context("reverse payment")?;
    ensure!(paid.status == 1, "reverse hold payment did not settle");
    println!("reverse claim settled RGB and real Lightning hold invoice without taker BTC");
    drop(maker);
    feed.abort();
    let mut maker_wallet = rgb!(open_wallet(
        &root().join("rgb-maker"),
        id["makerMnemonic"].as_str().context("maker mnemonic")?
    ))?;
    let mo = rgb!(online(&mut maker_wallet))?;
    let maker_remaining = INVENTORY + happy.expected_amount - lock.amount;
    wait_balance(&mut maker_wallet, mo, asset, maker_remaining).await?;
    ensure!(
        maker_remaining + remaining + lock.amount == ISSUED,
        "asset conservation"
    );
    let report = json!({"makerRevision":"49c6ce2e9554eea0d819fd3bbd267af6db45b1c0","rgbLibRevision":"96f039d975cf2a83712c3c0a90e703621d445425","network":"regtest","priceFeed":"deterministic BTC/USDT 100000 + 0.000001 per tick","assetId":asset,"issued":ISSUED,
        "submarine":{"id":happy.id,"lockTxid":happy_op.txid.to_string(),"amount":happy.expected_amount,"status":"transaction.claimed","lightningPaymentSucceeded":true},
        "refund":{"id":refund.id,"lockTxid":rop.txid.to_string(),"refundTxid":tx.compute_txid().to_string(),"amount":refund.expected_amount,"earlyRejectReason":"non-final","feeInputs":tx.input.len()-1,"rgbBalanceRestored":true},
        "reverse":{"id":reverse.id,"lockTxid":cop.txid.to_string(),"claimTxid":claim.compute_txid().to_string(),"amount":lock.amount,"htlcSat":lock.htlc_sat,"feeSat":lock.htlc_sat-claim.output[1].value.to_sat(),"payoutSat":claim.output[1].value.to_sat(),"walletBtcBefore":0,"inputCount":claim.input.len(),"status":"invoice.settled","lightningPaymentSucceeded":true},
        "settledBalances":{"maker":maker_remaining,"taker":remaining,"reverse":lock.amount,"total":ISSUED},"completedAtUnix":now()});
    private_json(&root().join("report.json"), &report)?;
    println!("all live checks passed; sanitized report: run/report.json");
    Ok(())
}
#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    let _ = rustls::crypto::ring::default_provider().install_default();
    match std::env::args().nth(1).as_deref() {
        Some("bootstrap") => bootstrap().await,
        Some("run") => run().await,
        Some("ts-bridge") => bridge::run().await,
        Some("ts-attach") => bridge::attach().await,
        Some("serve") => bridge::serve().await,
        Some("audit-amounts") => amount_audit::run().await,
        Some("audit-lightning") => amount_audit::lightning_balances().await,
        _ => bail!(
            "usage: rgb-sdk-regtest bootstrap|run|ts-bridge|ts-attach|serve|audit-amounts|audit-lightning"
        ),
    }
}
