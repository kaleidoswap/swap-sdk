//! The caller owns RGB state; the SDK never imports a wallet or creates a proof.
use crate::support::*;
use anyhow::{bail, ensure, Context, Result};
use bitcoin::{Address, Amount, OutPoint, Psbt, Sequence, Transaction, TxIn, TxOut, Witness};
use kaleidorg_swap_sdk::swaps::rgb::{
    ColoredRgbPsbt, FinalizedRgbSpend, PreparedRgbSpend, RgbAllocation, RgbLock,
};
use rgb_lib::{
    utils::script_buf_from_recipient_id,
    wallet::{
        rust_only::{AssetColoringInfo, ColoringInfo, PsbtOpExpiry, PsbtOpPrepareResult},
        Online, Recipient, RgbWalletOpsOffline, WitnessData,
    },
    Assignment, ContractId, Wallet,
};
use serde_json::json;
use std::{collections::HashMap, str::FromStr};

pub fn send(
    wallet: &mut Wallet,
    online: Online,
    asset: &str,
    recipient: Recipient,
) -> Result<String> {
    Ok(wallet
        .send(
            online,
            HashMap::from([(asset.into(), vec![recipient])]),
            true,
            2,
            1,
            now() + 3600,
            None,
        )?
        .txid)
}
pub fn lock(wallet: &mut Wallet, online: Online, instructions: &RgbLock) -> Result<String> {
    send(
        wallet,
        online,
        &instructions.asset_id,
        Recipient {
            recipient_id: instructions.recipient_id.clone(),
            assignment: Assignment::Fungible(instructions.amount),
            witness_data: Some(WitnessData {
                amount_sat: instructions.htlc_sat,
                blinding: Some(instructions.blinding.parse()?),
            }),
            transport_endpoints: instructions.transport_endpoints.clone(),
        },
    )
}
pub fn destination(wallet: &mut Wallet) -> Result<String> {
    let receive = wallet.witness_receive(None, Assignment::Any, now() + 3600, vec![], 1)?;
    let script = script_buf_from_recipient_id(receive.recipient_id)?.context("witness script")?;
    Ok(Address::from_script(&script, bitcoin::Network::Regtest)?.to_string())
}
pub fn fund(
    wallet: &mut Wallet,
    online: Online,
    prepared: PreparedRgbSpend,
) -> Result<PreparedRgbSpend> {
    let coin = wallet
        .list_unspents_vanilla(online, 1, false)?
        .into_iter()
        .filter(|u| u.txout.value.to_sat() > 10_000)
        .max_by_key(|u| u.txout.value)
        .context("confirmed wallet fee input")?;
    let mut psbt = Psbt::from_str(&prepared.template().psbt)?;
    psbt.unsigned_tx.input.push(TxIn {
        previous_output: coin.outpoint,
        script_sig: Default::default(),
        sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
        witness: Witness::new(),
    });
    psbt.inputs.push(bitcoin::psbt::Input {
        witness_utxo: Some(coin.txout.clone()),
        ..Default::default()
    });
    let change = Address::from_str(&wallet.get_address()?)?
        .require_network(bitcoin::Network::Regtest)?
        .script_pubkey();
    psbt.unsigned_tx.output.push(TxOut {
        value: Amount::from_sat(coin.txout.value.to_sat() - 2000),
        script_pubkey: change,
    });
    psbt.outputs.push(Default::default());
    sdk(prepared.fund(&psbt.to_string()))
}
pub fn color(
    wallet: &mut Wallet,
    prepared: &PreparedRgbSpend,
) -> Result<(PsbtOpPrepareResult, ColoredRgbPsbt)> {
    let template = prepared.template();
    let mut psbt = Psbt::from_str(&template.psbt)?;
    let operation = wallet.psbt_op_prepare_with_expiry(
        &mut psbt,
        ColoringInfo {
            asset_info_map: HashMap::from([(
                ContractId::from_str(&template.asset_id)?,
                AssetColoringInfo {
                    output_map: HashMap::from([(template.payment_output_index, template.amount)]),
                    static_blinding: Some(31337),
                },
            )]),
            static_blinding: Some(31337),
            nonce: None,
        },
        vec![OutPoint::from_str(&template.swap_outpoint)?],
        1,
        PsbtOpExpiry::Never,
    )?;
    // Read back actual assignments from rgb-lib's committed fascia. Never fabricate them
    // from the requested coloring, and never silently discard extra assignments.
    let allocations = operation
        .allocations
        .iter()
        .map(|a| match a.assignment {
            Assignment::Fungible(amount) => Ok(RgbAllocation {
                asset_id: a.asset_id.clone(),
                vout: a.vout,
                amount,
            }),
            _ => bail!("non-fungible assignment in USDT operation"),
        })
        .collect::<Result<Vec<_>>>()?;
    let colored = ColoredRgbPsbt {
        psbt: operation.colored_psbt.clone(),
        allocations,
    };
    Ok((operation, colored))
}
pub fn sign_wallet_inputs(
    wallet: &Wallet,
    finalized: FinalizedRgbSpend,
) -> Result<(Psbt, Transaction)> {
    let index = finalized.swap_input_index as usize;
    let witness = finalized.psbt.inputs[index].final_script_witness.clone();
    let signed = wallet.sign_psbt(finalized.psbt.to_string(), None)?;
    let psbt = Psbt::from_str(&wallet.finalize_psbt(signed, None)?)?;
    ensure!(
        psbt.inputs[index].final_script_witness == witness,
        "wallet changed SDK HTLC witness"
    );
    ensure!(
        psbt.inputs.iter().all(|i| i.final_script_witness.is_some()),
        "wallet did not finalize all inputs"
    );
    let transaction = psbt.clone().extract_tx()?;
    Ok((psbt, transaction))
}
pub async fn broadcast_apply(
    wallet: &mut Wallet,
    online: Online,
    operation: &PsbtOpPrepareResult,
    asset: &str,
    psbt: &Psbt,
    tx: &Transaction,
) -> Result<()> {
    let txid = tx.compute_txid().to_string();
    // Durable state precedes the attempt; an uncertain broadcast is recoverable by txid.
    private_json(
        &root().join(format!("operation-{}.json", operation.operation_id)),
        &json!({
            "operationId":operation.operation_id,"operationDir":operation.operation_dir,"psbt":psbt.to_string(),"txid":txid,"assetId":asset,
        }),
    )?;
    tokio::task::block_in_place(|| wallet.psbt_op_mark_broadcast(&operation.operation_id))?;
    ensure!(
        rpc(
            "sendrawtransaction",
            json!([hex::encode(bitcoin::consensus::serialize(tx))])
        )
        .await?
            == txid,
        "broadcast txid mismatch"
    );
    mine(1).await?;
    tokio::task::block_in_place(|| wallet.psbt_op_apply(online, &operation.operation_id))?;
    tokio::task::block_in_place(|| {
        wallet.psbt_op_provide_receive_consignment(online, &operation.operation_id, asset)
    })?;
    mine(1).await?;
    Ok(())
}
