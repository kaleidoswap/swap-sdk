//! Private JSON-line wallet/Lightning bridge. Swap creation and HTLC signing live in TypeScript.
use crate::{identity, pricefeed, support::*, wallet};
use anyhow::{bail, ensure, Context, Result};
use bitcoin::{consensus, Psbt, Transaction};
use kaleidorg_swap_sdk::swaps::rgb::{FinalizedRgbSpend, RgbLock, RgbPsbtTemplate};
use ldk_server_client::ldk_server_grpc::api::*;
use rgb_lib::{
    wallet::{
        rust_only::{ExpectedTransfer, PsbtOpPrepareResult},
        Online, RgbWalletOpsOffline, RgbWalletOpsOnline,
    },
    AssetSchema, Assignment, Wallet,
};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    io::{BufRead, Write},
    str::FromStr,
};

struct Bridge {
    taker: Wallet,
    receiver: Wallet,
    to: Online,
    ro: Online,
    asset: String,
    operations: HashMap<String, PsbtOpPrepareResult>,
}
fn field<'a>(v: &'a Value, key: &str) -> Result<&'a str> {
    v[key].as_str().with_context(|| format!("missing {key}"))
}
impl Bridge {
    async fn command(&mut self, v: &Value) -> Result<Value> {
        let name = field(v, "command")?;
        let receiver = v["wallet"] == "reverse";
        let online = if receiver { self.ro } else { self.to };
        let w = if receiver {
            &mut self.receiver
        } else {
            &mut self.taker
        };
        match name {
            "info" => Ok(json!({"assetId":self.asset,"issued":ISSUED,"inventory":INVENTORY})),
            "persist" => {
                let tag = field(v, "tag")?;
                ensure!(
                    tag.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'),
                    "invalid tag"
                );
                private_json(&root().join(format!("ts-{tag}.json")), &v["state"])?;
                Ok(json!(true))
            }
            "invoice" => {
                let ln = node("taker").await?;
                if let Some(hash) = v["hash"].as_str() {
                    let i = ln
                        .bolt11_receive_for_hash(Bolt11ReceiveForHashRequest {
                            amount_msat: Some(100_000_000),
                            description: None,
                            expiry_secs: 3600,
                            payment_hash: hash.into(),
                            min_final_cltv_expiry_delta: None,
                        })
                        .await?;
                    Ok(json!({"invoice":i.invoice,"paymentId":hash}))
                } else {
                    let i = ln
                        .bolt11_receive(Bolt11ReceiveRequest {
                            amount_msat: Some(100_000_000),
                            description: None,
                            expiry_secs: 3600,
                        })
                        .await?;
                    Ok(json!({"invoice":i.invoice,"paymentId":i.payment_hash}))
                }
            }
            "pay" => Ok(
                json!({"paymentId":node("taker").await?.bolt11_send(Bolt11SendRequest {invoice:field(v,"invoice")?.into(),amount_msat:None,route_parameters:None}).await?.payment_id}),
            ),
            "failHold" => {
                node("taker")
                    .await?
                    .bolt11_fail_for_hash(Bolt11FailForHashRequest {
                        payment_hash: field(v, "hash")?.into(),
                    })
                    .await?;
                Ok(json!(true))
            }
            "payment" => {
                let p = node("taker")
                    .await?
                    .get_payment_details(GetPaymentDetailsRequest {
                        payment_id: field(v, "paymentId")?.into(),
                    })
                    .await?
                    .payment
                    .context("payment")?;
                Ok(json!({"status":p.status}))
            }
            "lock" => {
                let lock: RgbLock = serde_json::from_value(v["lock"].clone())?;
                ensure!(
                    lock.asset_id == self.asset && lock.htlc_sat <= 1000,
                    "wallet lock policy"
                );
                let txid = tokio::task::block_in_place(|| wallet::lock(w, online, &lock))?;
                mine(1).await?;
                let tx = transaction(&txid).await?;
                Ok(json!({"txid":txid,"hex":hex::encode(consensus::serialize(&tx))}))
            }
            "destination" => Ok(json!(tokio::task::block_in_place(|| wallet::destination(
                w
            ))?)),
            "fund" => {
                let t: RgbPsbtTemplate = serde_json::from_value(v["template"].clone())?;
                Ok(json!(tokio::task::block_in_place(|| {
                    wallet::fund_template(w, online, &t)
                })?))
            }
            "color" => {
                let t: RgbPsbtTemplate = serde_json::from_value(v["template"].clone())?;
                let (op, colored) = tokio::task::block_in_place(|| wallet::color_template(w, &t))?;
                let id = op.operation_id.clone();
                private_json(
                    &root().join(format!("ts-operation-{id}.json")),
                    &json!({"operationId":id,"operationDir":op.operation_dir,"wallet":v["wallet"],"colored":colored}),
                )?;
                self.operations.insert(id.clone(), op);
                Ok(json!({"operationId":id,"colored":colored}))
            }
            "accept" => {
                let lock: RgbLock = serde_json::from_value(v["lock"].clone())?;
                ensure!(lock.asset_id == self.asset, "asset pin");
                let blinding = lock.blinding.parse()?;
                let (consignment, assignments) = tokio::task::block_in_place(|| {
                    w.fetch_and_accept_transfer_by_recipient_id(
                        online,
                        lock.recipient_id.clone(),
                        lock.recipient_id.clone(),
                        &lock.transport_endpoints[0],
                        blinding,
                        lock.min_confirmations,
                        ExpectedTransfer {
                            asset_id: self.asset.clone(),
                            asset_schema: AssetSchema::Nia,
                            assignment: Assignment::Fungible(lock.amount),
                        },
                    )
                })?;
                ensure!(
                    assignments == vec![Assignment::Fungible(lock.amount)],
                    "consignment allocation"
                );
                let txid = field(v, "txid")?.to_owned();
                tokio::task::block_in_place(|| w.save_new_asset(online, consignment, txid))?;
                Ok(json!(true))
            }
            "restoreOperation" => {
                let id = field(v, "operationId")?;
                ensure!(
                    id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'),
                    "invalid operation ID"
                );
                ensure!(
                    w.psbt_op_reconcile(id)?
                        == rgb_lib::wallet::rust_only::PsbtOperationStatus::Prepared,
                    "only an unbroadcast prepared operation can resume this signing test"
                );
                let saved: Value = serde_json::from_slice(&std::fs::read(
                    root().join(format!("ts-operation-{id}.json")),
                )?)?;
                let colored = saved["colored"].clone();
                let op = PsbtOpPrepareResult {
                    operation_id: id.into(),
                    operation_dir: field(&saved, "operationDir")?.into(),
                    colored_psbt: field(&colored, "psbt")?.into(),
                    allocations: vec![],
                };
                self.operations.insert(id.into(), op);
                let mut funded = Psbt::from_str(field(&colored, "psbt")?)?;
                let address = bitcoin::Address::from_script(
                    &funded.unsigned_tx.output[1].script_pubkey,
                    bitcoin::Network::Regtest,
                )?
                .to_string();
                funded.unsigned_tx.output[0].script_pubkey =
                    bitcoin::ScriptBuf::from_bytes(vec![0x6a, 0x00]);
                Ok(
                    json!({"operationId":id,"colored":colored,"fundedPsbt":funded.to_string(),"outputAddress":address}),
                )
            }
            "validateRefund" => {
                let psbt = Psbt::from_str(field(v, "psbt")?)?;
                let outpoint = bitcoin::OutPoint::new(
                    bitcoin::Txid::from_str(field(v, "txid")?)?,
                    v["vout"].as_u64().context("vout")?.try_into()?,
                );
                let amount = v["amount"].as_u64().context("amount")?;
                let contract = rgb_lib::ContractId::from_str(&self.asset)?;
                tokio::task::block_in_place(|| {
                    w.validate_htlc_spend(&psbt, contract, outpoint, amount, 1, ESPLORA)
                })?;
                Ok(json!(true))
            }
            "mutatePsbt" => {
                let mut psbt = Psbt::from_str(field(v, "psbt")?)?;
                match field(v, "mutation")? {
                    "commitment" => {
                        psbt.unsigned_tx.output[0].script_pubkey = bitcoin::ScriptBuf::from_bytes(
                            [vec![0x6a, 0x20], vec![0u8; 32]].concat(),
                        )
                    }
                    "prevout" => {
                        psbt.inputs[v["index"].as_u64().context("index")? as usize]
                            .witness_utxo
                            .as_mut()
                            .context("prevout")?
                            .value += bitcoin::Amount::from_sat(1)
                    }
                    "proof" => psbt.proprietary.clear(),
                    _ => bail!("unknown mutation"),
                }
                Ok(json!(psbt.to_string()))
            }
            "signWallet" => {
                let finalized = FinalizedRgbSpend {
                    psbt: Psbt::from_str(field(v, "psbt")?)?,
                    swap_input_index: v["swapInputIndex"].as_u64().context("index")?.try_into()?,
                    transaction: None,
                };
                let (psbt, tx) =
                    tokio::task::block_in_place(|| wallet::sign_wallet_inputs(w, finalized))?;
                Ok(
                    json!({"psbt":psbt.to_string(),"hex":hex::encode(consensus::serialize(&tx)),"txid":tx.compute_txid().to_string()}),
                )
            }
            "broadcast" => {
                let op = self
                    .operations
                    .get(field(v, "operationId")?)
                    .context("operation")?;
                let psbt = Psbt::from_str(field(v, "psbt")?)?;
                let tx: Transaction = consensus::deserialize(&hex::decode(field(v, "hex")?)?)?;
                wallet::broadcast_apply(w, online, op, &self.asset, &psbt, &tx).await?;
                Ok(json!(tx.compute_txid().to_string()))
            }
            "balance" => {
                if let Some(expected) = v["expected"].as_u64() {
                    wait_balance(w, online, &self.asset, expected).await?;
                }
                tokio::task::block_in_place(|| -> Result<Value> {
                    w.refresh(online, None, vec![], false)?;
                    let btc = w.get_btc_balance(Some(online), false)?;
                    let rgb = w
                        .get_asset_balance(self.asset.clone())
                        .ok()
                        .map(|b| b.settled)
                        .unwrap_or(0);
                    Ok(json!({"rgb":rgb,"btc":btc.vanilla.settled+btc.colored.settled}))
                })
            }
            "inspect" => {
                let tx: Transaction = consensus::deserialize(&hex::decode(field(v, "hex")?)?)?;
                let mut input_sat = 0;
                for i in &tx.input {
                    let prev = transaction(&i.previous_output.txid.to_string()).await?;
                    input_sat += prev.output[i.previous_output.vout as usize].value.to_sat();
                }
                let output_sat: u64 = tx.output.iter().map(|o| o.value.to_sat()).sum();
                let revealed = v["preimage"]
                    .as_str()
                    .map(|p| {
                        hex::decode(p).map(|p| tx.input[0].witness.nth(1) == Some(p.as_slice()))
                    })
                    .transpose()?;
                Ok(
                    json!({"txid":tx.compute_txid().to_string(),"inputCount":tx.input.len(),"witnessLength":tx.input[0].witness.len(),"feeSat":input_sat-output_sat,"payoutSat":tx.output[1].value.to_sat(),"locktime":tx.lock_time.to_consensus_u32(),"hasRgbCommitment":tx.output[0].script_pubkey.is_op_return()&&tx.output[0].script_pubkey.len()>2,"preimageMatches":revealed}),
                )
            }
            "mempool" => rpc("testmempoolaccept", json!([[field(v, "hex")?]])).await,
            "mine" => {
                mine(v["blocks"].as_u64().context("blocks")?.try_into()?).await?;
                Ok(json!(true))
            }
            "tip" => Ok(json!(tip().await?)),
            _ => bail!("unknown command {name}"),
        }
    }
}
pub async fn serve() -> Result<()> {
    let id = identity()?;
    let _feed = pricefeed::start().await?;
    let _maker = MakerProcess::start(&id).await?;
    println!("RGB maker serving at {MAKER}");
    private_json(
        &root().join("server-process.json"),
        &json!({"supervisorPid":std::process::id(),"makerUrl":MAKER}),
    )?;
    #[cfg(unix)]
    {
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
        tokio::select! { _ = terminate.recv() => {}, _ = tokio::signal::ctrl_c() => {} }
    }
    #[cfg(not(unix))]
    tokio::signal::ctrl_c().await?;
    _feed.abort();
    Ok(())
}
pub async fn run() -> Result<()> {
    let id = identity()?;
    ensure!(
        !root().join("ts-started.json").exists(),
        "TS run already attempted: preserve recovery state before retrying"
    );
    private_json(&root().join("ts-started.json"), &json!({"started":now()}))?;
    let feed = pricefeed::start().await?;
    let maker = MakerProcess::start(&id).await?;
    script(&["publish-pairs"])?;
    drop(maker);
    let maker = MakerProcess::start(&id).await?;
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    let mut taker = tokio::task::block_in_place(|| {
        open_wallet(&root().join("rgb-taker"), field(&id, "takerMnemonic")?)
    })?;
    let mut receiver = tokio::task::block_in_place(|| {
        open_wallet(&root().join("rgb-reverse"), field(&id, "reverseMnemonic")?)
    })?;
    let to = tokio::task::block_in_place(|| online(&mut taker))?;
    let ro = tokio::task::block_in_place(|| online(&mut receiver))?;
    let mut b = Bridge {
        taker,
        receiver,
        to,
        ro,
        asset: field(&id, "assetId")?.into(),
        operations: HashMap::new(),
    };
    let mut lines = std::io::stdin().lock().lines();
    loop {
        let line = tokio::task::block_in_place(|| lines.next());
        let Some(line) = line else { break };
        let v: Value = serde_json::from_str(&line?)?;
        if v["command"] == "audit" {
            drop(maker);
            let mut w = tokio::task::block_in_place(|| {
                open_wallet(&root().join("rgb-maker"), field(&id, "makerMnemonic")?)
            })?;
            let online = tokio::task::block_in_place(|| online(&mut w))?;
            wait_balance(
                &mut w,
                online,
                &b.asset,
                v["expected"].as_u64().context("expected")?,
            )
            .await?;
            let balance =
                tokio::task::block_in_place(|| w.get_asset_balance(b.asset.clone()))?.settled;
            println!("{}", json!({"id":v["id"],"result":{"maker":balance}}));
            std::io::stdout().flush()?;
            break;
        }
        let response = match b.command(&v).await {
            Ok(result) => json!({"id":v["id"],"result":result}),
            Err(e) => json!({"id":v["id"],"error":format!("{e:#}")}),
        };
        println!("{response}");
        std::io::stdout().flush()?;
    }
    feed.abort();
    Ok(())
}

/// Attach a wallet bridge to an already supervised maker without restarting or reseeding it.
pub async fn attach() -> Result<()> {
    let id = identity()?;
    let mut taker = tokio::task::block_in_place(|| {
        open_wallet(&root().join("rgb-taker"), field(&id, "takerMnemonic")?)
    })?;
    let mut receiver = tokio::task::block_in_place(|| {
        open_wallet(&root().join("rgb-reverse"), field(&id, "reverseMnemonic")?)
    })?;
    let to = tokio::task::block_in_place(|| online(&mut taker))?;
    let ro = tokio::task::block_in_place(|| online(&mut receiver))?;
    let mut bridge = Bridge {
        taker,
        receiver,
        to,
        ro,
        asset: field(&id, "assetId")?.into(),
        operations: HashMap::new(),
    };
    let mut lines = std::io::stdin().lock().lines();
    loop {
        let Some(line) = tokio::task::block_in_place(|| lines.next()) else {
            break;
        };
        let value: Value = serde_json::from_str(&line?)?;
        let reply = match bridge.command(&value).await {
            Ok(result) => json!({"id":value["id"],"result":result}),
            Err(error) => json!({"id":value["id"],"error":format!("{error:#}")}),
        };
        println!("{reply}");
        std::io::stdout().flush()?;
    }
    Ok(())
}
