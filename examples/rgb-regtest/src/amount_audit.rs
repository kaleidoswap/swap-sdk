//! Reconcile actual RGB transitions, spendable allocations and both Lightning peers.
use crate::{identity, support::*};
use anyhow::{ensure, Context, Result};
use ldk_server_client::{ldk_server_grpc::api::*, ldk_server_grpc::types::payment_kind};
use rgb_lib::{
    wallet::{AssetFilter, RgbWalletOpsOffline, RgbWalletOpsOnline},
    Assignment,
};
use serde_json::{json, Value};
use std::fs;
fn fungible(a: &Assignment) -> Result<u64> {
    match a {
        Assignment::Fungible(amount) => Ok(*amount),
        _ => anyhow::bail!("unexpected non-fungible assignment"),
    }
}
pub async fn run() -> Result<()> {
    let id = identity()?;
    let asset = id["assetId"].as_str().context("asset")?;
    let report: Value = serde_json::from_slice(&fs::read(root().join("ts-report.json"))?)?;
    let mut audit = json!({"assetId":asset,"rgbDecimals":6,"wallets":{},"lightning":{},"completedAtUnix":now()});
    let mut total = 0;
    for (name, mnemonic) in [
        ("maker", "makerMnemonic"),
        ("taker", "takerMnemonic"),
        ("reverse", "reverseMnemonic"),
    ] {
        let mut w = tokio::task::block_in_place(|| {
            open_wallet(
                &root().join(format!("rgb-{name}")),
                id[mnemonic].as_str().context("mnemonic")?,
            )
        })?;
        let online = tokio::task::block_in_place(|| online(&mut w))?;
        let result = tokio::task::block_in_place(|| -> Result<Value> {
            w.refresh(online, None, vec![], false)?;
            let balance = w.get_asset_balance(asset.into())?;
            let unspents = w.list_unspents(Some(online), true, false)?;
            let mut allocated = 0;
            let mut allocations = vec![];
            for u in unspents {
                for a in u
                    .rgb_allocations
                    .iter()
                    .filter(|a| a.asset_id.as_deref() == Some(asset))
                {
                    ensure!(a.settled, "unsettled output");
                    let amount = fungible(&a.assignment)?;
                    allocated += amount;
                    allocations.push(json!({"outpoint":format!("{}:{}",u.utxo.outpoint.txid,u.utxo.outpoint.vout),"amount":amount}));
                }
            }
            ensure!(
                allocated == balance.settled,
                "balance does not match spendable assignments"
            );
            ensure!(
                balance.settled
                    == report["settledBalances"][name]
                        .as_u64()
                        .context("expected")?,
                "wallet differs from initial audit"
            );
            let transfers = w.list_transfers(AssetFilter::Id(asset.into()), None)?;
            let transfers=transfers.into_iter().map(|t| -> Result<Value> {Ok(json!({"txid":t.txid,"kind":format!("{:?}",t.kind),"status":format!("{:?}",t.status),"amounts":t.assignments.iter().map(fungible).collect::<Result<Vec<_>>>()?}))}).collect::<Result<Vec<_>>>()?;
            let spends = if name == "maker" {
                vec!["submarine", "reverseRefund"]
            } else if name == "taker" {
                vec!["submarineRefund"]
            } else {
                vec!["reverse"]
            };
            let mut operations = vec![];
            for flow in spends {
                let txid = if flow == "submarine" {
                    let mut found = None;
                    for entry in fs::read_dir(w.get_wallet_dir().join("psbt_ops"))? {
                        let path = entry?.path().join("meta.json");
                        let meta: Value = serde_json::from_slice(&fs::read(path)?)?;
                        let txid = meta["txid"].as_str().context("txid")?;
                        if txid != report["reverseRefund"]["txid"].as_str().context("refund")? {
                            found = Some(txid.to_owned());
                        }
                    }
                    found.context("claim operation")?
                } else {
                    report[flow]["txid"].as_str().context("spend")?.into()
                };
                let op = w.psbt_op_by_txid(&txid)?;
                ensure!(op.allocations.len() == 1, "unexpected extra RGB allocation");
                let a = &op.allocations[0];
                ensure!(
                    a.asset_id == asset && a.vout == Some(1),
                    "wrong output or asset"
                );
                let amount = fungible(&a.assignment)?;
                ensure!(
                    amount == report[flow]["amount"].as_u64().context("quoted")?,
                    "quoted amount differs from actual fascia"
                );
                operations.push(json!({"flow":flow,"txid":txid,"amount":amount,"vout":a.vout,"status":format!("{:?}",op.status)}));
            }
            Ok(
                json!({"settled":balance.settled,"spendableAllocationSum":allocated,"allocations":allocations,"transfers":transfers,"actualTransitionAllocations":operations}),
            )
        })?;
        total += result["settled"].as_u64().context("balance")?;
        audit["wallets"][name] = result;
    }
    ensure!(total == ISSUED, "RGB units missing");
    audit["rgbTotal"] = json!(total);
    for name in ["maker", "taker"] {
        let ln = node(name).await?;
        let mut token = None;
        let mut payments = vec![];
        loop {
            let page = ln
                .list_payments(ListPaymentsRequest { page_token: token })
                .await?;
            for p in page.payments {
                if let Some(payment_kind::Kind::Bolt11(b)) = p.kind.and_then(|k| k.kind) {
                    payments.push(json!({"paymentId":p.id,"hash":b.hash,"amountMsat":p.amount_msat,"feePaidMsat":p.fee_paid_msat,"direction":p.direction,"status":p.status}));
                }
            }
            token = page.next_page_token;
            if token.is_none() {
                break;
            }
        }
        let channels = ln.list_channels(ListChannelsRequest {}).await?;
        audit["lightning"][name] = json!({"payments":payments,"usableChannels":channels.channels.iter().filter(|c|c.is_usable).count()});
    }
    let query = "SELECT json_agg(json_build_object('id',o.id,'kind',o.kind,'state',o.state,'wireStatus',o.wire_status,'fromAmount',o.from_amount,'toAmount',o.to_amount,'protocolFee',o.quoted_protocol_fee,'networkFee',o.quoted_network_fee,'serviceFee',o.quoted_swap_fee,'pairHash',encode(o.pair_hash,'hex'),'rgbAmount',l.amount,'htlcSat',l.htlc_sat,'lockTxid',l.lock_txid,'lockVout',l.lock_vout,'spendTxid',l.spend_txid) ORDER BY o.created_at) FROM swap_orders o JOIN rgb_htlc_lockups l ON l.swap_id=o.id;";
    let snapshot = std::process::Command::new("docker")
        .args([
            "exec",
            "rgb-sdk-regtest-postgres-1",
            "psql",
            "-U",
            "postgres",
            "-d",
            "maker",
            "-t",
            "-A",
            "-c",
            query,
        ])
        .output()?;
    ensure!(
        snapshot.status.success(),
        "failed to read isolated maker amount snapshot"
    );
    let orders: Value = serde_json::from_slice(&snapshot.stdout)?;
    private_json(&root().join("amount-audit-orders.json"), &orders)?;
    private_json(&root().join("amount-audit-wallets.json"), &audit)?;
    println!("RGB actual transition allocations, wallet UTXOs and Lightning amounts captured in private run/amount-audit-wallets.json");
    Ok(())
}

pub async fn lightning_balances() -> Result<()> {
    use ldk_server_client::ldk_server_grpc::types::lightning_balance::BalanceType;
    let mut result = json!({});
    let mut claimable_and_fees = 0;
    let mut funding_value = None;
    for name in ["maker", "taker"] {
        let ln = node(name).await?;
        let balances = ln.get_balances(GetBalancesRequest {}).await?;
        ensure!(
            balances.pending_balances_from_channel_closures.is_empty(),
            "channel closure pending"
        );
        let mut closed = vec![];
        for b in balances.lightning_balances {
            let Some(BalanceType::ClaimableOnChannelClose(b)) = b.balance_type else {
                anyhow::bail!("unresolved Lightning balance or HTLC");
            };
            ensure!(
                b.outbound_payment_htlc_rounded_msat == 0
                    && b.outbound_forwarded_htlc_rounded_msat == 0
                    && b.inbound_claiming_htlc_rounded_msat == 0
                    && b.inbound_htlc_rounded_msat == 0,
                "HTLC dust rounding loss"
            );
            claimable_and_fees += b.amount_satoshis + b.transaction_fee_satoshis;
            closed.push(json!({"claimableSat":b.amount_satoshis,"commitmentFeeSat":b.transaction_fee_satoshis,"htlcRoundingLossMsat":0}));
        }
        let channels = ln.list_channels(ListChannelsRequest {}).await?.channels;
        ensure!(
            channels.len() == 1 && channels[0].is_usable,
            "channel not usable"
        );
        let channel = &channels[0];
        if let Some(value) = funding_value {
            ensure!(
                value == channel.channel_value_sats,
                "peer channel values differ"
            );
        } else {
            funding_value = Some(channel.channel_value_sats);
        }
        result[name] = json!({"totalClaimableSat":balances.total_lightning_balance_sats,"closeBalances":closed,"outboundCapacityMsat":channel.outbound_capacity_msat,"inboundCapacityMsat":channel.inbound_capacity_msat});
    }
    private_json(
        &root().join("amount-audit-lightning-balances.json"),
        &result,
    )?;
    // The default LDK anchor channel has two 330-sat anchor outputs. These
    // are excluded from both peers' ClaimableOnChannelClose and miner-fee figures.
    let anchor_outputs_sat = 2 * 330;
    ensure!(
        Some(claimable_and_fees + anchor_outputs_sat) == funding_value,
        "channel funding value differs: claimable plus fees={claimable_and_fees}, funding={funding_value:?}"
    );
    result["channelFundingSat"] = json!(funding_value);
    result["claimablePlusCommitmentFeesSat"] = json!(claimable_and_fees);
    result["anchorOutputsSat"] = json!(anchor_outputs_sat);
    result["unaccountedSat"] = json!(0);
    result["unresolvedHtlcs"] = json!(0);
    private_json(
        &root().join("amount-audit-lightning-balances.json"),
        &result,
    )?;
    println!(
        "Lightning channel value fully reconciled; no unresolved HTLCs or millisatoshi dust loss"
    );
    Ok(())
}
