use bitcoin::{
    hashes::{hash160, Hash},
    hex::DisplayHex,
    key::rand::{rngs::OsRng, RngCore},
    secp256k1::Keypair,
    Amount, Witness, XOnlyPublicKey,
};
use elements::{
    confidential::{Asset, AssetBlindingFactor, Value, ValueBlindingFactor},
    hex::FromHex,
    pset::{Input as PsetInput, Output as PsetOutput, PartiallySignedTransaction},
    secp256k1_zkp::{Secp256k1, SecretKey},
    sighash::{Prevouts, SighashCache},
    taproot::{LeafVersion, TapLeafHash, TaprootBuilder, TaprootSpendInfo},
    Address, AssetIssuance, BlindAssetProofs, BlindValueProofs, BlockHash, LockTime, OutPoint,
    SchnorrSig, SchnorrSighashType, Script, Sequence, Transaction, TxIn, TxInWitness, TxOut,
    TxOutSecrets, TxOutWitness,
};
use secp256k1_musig::{musig, Scalar};
use std::collections::HashSet;
use std::str::FromStr;

use elements::encode::serialize;
use elements::secp256k1_zkp::Message;

use crate::util::{
    hex_to_bytes32,
    secrets::{rng_32b, Preimage},
};

use crate::error::Error;

use super::{
    boltz::{
        BoltzApiClientV2, ChainSwapDetails, Cooperative, CreateReverseResponse,
        CreateSubmarineResponse, Side, SwapTxKind, SwapType, ToSign,
    },
    wrappers::SwapScriptCommon,
};
use crate::fees::{create_tx_with_fee, Fee};
use crate::network::{Currency, LiquidChain, LiquidClient};
use elements::bitcoin::PublicKey;
use elements::secp256k1_zkp::Keypair as ZKKeyPair;
use elements::{
    address::Address as EAddress,
    opcodes::all::*,
    script::{Builder as EBuilder, Instruction},
};

pub(crate) fn find_utxo(tx: &Transaction, script_pubkey: &Script) -> Option<(OutPoint, TxOut)> {
    for (vout, output) in tx.clone().output.into_iter().enumerate() {
        if output.script_pubkey == *script_pubkey {
            let outpoint = OutPoint::new(tx.txid(), vout as u32);
            return Some((outpoint, output));
        }
    }
    None
}

/// Swap and fee assets resolved for a Liquid response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiquidAssetContext {
    pub swap_asset: elements::AssetId,
    pub policy_asset: elements::AssetId,
}

impl LiquidAssetContext {
    fn from_response(
        asset_id: Option<&str>,
        fee_asset_id: Option<&str>,
    ) -> Result<Option<Self>, Error> {
        match (asset_id, fee_asset_id) {
            (None, None) => Ok(None),
            (Some(asset_id), Some(fee_asset_id)) => Ok(Some(Self {
                swap_asset: elements::AssetId::from_str(asset_id)?,
                policy_asset: elements::AssetId::from_str(fee_asset_id)?,
            })),
            _ => Err(Error::Protocol(
                "Liquid responses must provide assetId and feeAssetId together".to_string(),
            )),
        }
    }

    pub fn legacy_lbtc(network: LiquidChain) -> Self {
        let policy_asset = network.bitcoin();
        Self {
            swap_asset: policy_asset,
            policy_asset,
        }
    }

    pub fn is_policy_asset_swap(self) -> bool {
        self.swap_asset == self.policy_asset
    }
}

/// Decode and validate a Liquid HTLC output for the expected swap asset.
///
/// V1 accepts either a fully explicit asset/value pair without a blinding key,
/// or a fully confidential pair with its blinding key. Mixed encodings are
/// rejected.
pub fn decode_swap_output(
    txout: &TxOut,
    blinding_key: Option<SecretKey>,
    expected_asset: elements::AssetId,
) -> Result<TxOutSecrets, Error> {
    let secrets = match (&txout.asset, &txout.value, blinding_key) {
        (Asset::Explicit(asset), Value::Explicit(value), None) => TxOutSecrets {
            asset: *asset,
            asset_bf: AssetBlindingFactor::zero(),
            value: *value,
            value_bf: ValueBlindingFactor::zero(),
        },
        (Asset::Confidential(_), Value::Confidential(_), Some(blinding_key)) => {
            txout.unblind(&Secp256k1::new(), blinding_key)?
        }
        (Asset::Explicit(_), Value::Explicit(_), Some(_)) => {
            return Err(Error::Protocol(
                "Explicit Liquid swap output must not have a blinding key".to_string(),
            ));
        }
        (Asset::Confidential(_), Value::Confidential(_), None) => {
            return Err(Error::Protocol(
                "Confidential Liquid swap output requires a blinding key".to_string(),
            ));
        }
        _ => {
            return Err(Error::Protocol(
                "Mixed explicit/confidential Liquid swap output is unsupported".to_string(),
            ));
        }
    };

    if secrets.asset != expected_asset {
        return Err(Error::Protocol(format!(
            "Liquid swap asset mismatch: expected {expected_asset}, got {}",
            secrets.asset
        )));
    }
    Ok(secrets)
}

/// Liquid v2 swap script helper.
#[derive(Debug, Clone, PartialEq)]
pub struct LiquidSwapScript {
    pub swap_type: SwapType,
    pub side: Option<Side>,
    pub funding_addrs: Option<Address>,
    pub hashlock: hash160::Hash,
    pub receiver_pubkey: PublicKey,
    pub locktime: LockTime,
    pub sender_pubkey: PublicKey,
    pub blinding_key: Option<ZKKeyPair>,
    /// Explicit server asset ids; absent only for legacy L-BTC responses.
    pub asset_context: Option<LiquidAssetContext>,
    /// Exact amount expected at the swap HTLC output.
    pub expected_amount: u64,
}

/// Deprecated compatibility alias. Use [`LiquidSwapScript`].
#[deprecated(since = "0.4.2", note = "renamed to LiquidSwapScript")]
pub type LBtcSwapScript = LiquidSwapScript;

/// Wallet-neutral template for a caller-funded Liquid spend.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiquidPsetTemplate {
    pub pset: String,
    pub swap_input_index: u32,
    pub payment_output_index: u32,
    pub swap_asset_id: String,
    pub policy_asset_id: String,
    pub amount: u64,
    pub max_fee: u64,
}

/// Unblinded data for the payment output returned by the caller's wallet.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiquidOutputSecrets {
    pub asset_id: String,
    pub value: u64,
    pub asset_blinding_factor: String,
    pub value_blinding_factor: String,
}

/// A wallet-funded, blinded and wallet-signed PSET ready for swap finalization.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FundedLiquidPset {
    pub pset: String,
    pub payment_output_secrets: LiquidOutputSecrets,
}

impl LiquidSwapScript {
    /// Create the struct for a submarine swap from boltz create response.
    pub fn submarine_from_swap_resp(
        create_swap_response: &CreateSubmarineResponse,
        our_pubkey: PublicKey,
    ) -> Result<Self, Error> {
        let claim_script = Script::from_hex(&create_swap_response.swap_tree.claim_leaf.output)?;
        let refund_script = Script::from_hex(&create_swap_response.swap_tree.refund_leaf.output)?;

        let claim_instructions = claim_script.instructions();
        let refund_instructions = refund_script.instructions();

        let mut last_op = OP_0NOTEQUAL;
        let mut hashlock = None;
        let mut locktime = None;

        for instruction in claim_instructions {
            match instruction {
                Ok(Instruction::PushBytes(bytes)) => {
                    if bytes.len() == 20 {
                        hashlock = Some(hash160::Hash::from_slice(bytes)?);
                    } else {
                        continue;
                    }
                }
                _ => continue,
            }
        }

        for instruction in refund_instructions {
            match instruction {
                Ok(Instruction::Op(opcode)) => last_op = opcode,
                Ok(Instruction::PushBytes(bytes)) => {
                    if last_op == OP_CHECKSIGVERIFY {
                        locktime =
                            Some(LockTime::from_consensus(bytes_to_u32_little_endian(bytes)));
                    } else {
                        continue;
                    }
                }
                _ => continue,
            }
        }

        let hashlock =
            hashlock.ok_or_else(|| Error::Protocol("No hashlock provided".to_string()))?;

        let locktime =
            locktime.ok_or_else(|| Error::Protocol("No timelock provided".to_string()))?;

        let funding_addrs = Address::from_str(&create_swap_response.address)?;

        let blinding_key = create_swap_response
            .blinding_key
            .as_deref()
            .map(|key| ZKKeyPair::from_seckey_str(&Secp256k1::new(), key))
            .transpose()?;
        let asset_context = LiquidAssetContext::from_response(
            create_swap_response.asset_id.as_deref(),
            create_swap_response.fee_asset_id.as_deref(),
        )?;

        Ok(Self {
            swap_type: SwapType::Submarine,
            side: None,
            funding_addrs: Some(funding_addrs),
            hashlock,
            receiver_pubkey: create_swap_response.claim_public_key,
            locktime,
            sender_pubkey: our_pubkey,
            blinding_key,
            asset_context,
            expected_amount: create_swap_response.expected_amount,
        })
    }

    /// Create the struct for a reverse swap from boltz create response.
    pub fn reverse_from_swap_resp(
        reverse_response: &CreateReverseResponse,
        our_pubkey: PublicKey,
    ) -> Result<Self, Error> {
        let claim_script = Script::from_hex(&reverse_response.swap_tree.claim_leaf.output)?;
        let refund_script = Script::from_hex(&reverse_response.swap_tree.refund_leaf.output)?;

        let claim_instructions = claim_script.instructions();
        let refund_instructions = refund_script.instructions();

        let mut last_op = OP_0NOTEQUAL;
        let mut hashlock = None;
        let mut locktime = None;

        for instruction in claim_instructions {
            match instruction {
                Ok(Instruction::PushBytes(bytes)) => {
                    if bytes.len() == 20 {
                        hashlock = Some(hash160::Hash::from_slice(bytes)?);
                    } else {
                        continue;
                    }
                }
                _ => continue,
            }
        }

        for instruction in refund_instructions {
            match instruction {
                Ok(Instruction::Op(opcode)) => last_op = opcode,
                Ok(Instruction::PushBytes(bytes)) => {
                    if last_op == OP_CHECKSIGVERIFY {
                        locktime =
                            Some(LockTime::from_consensus(bytes_to_u32_little_endian(bytes)));
                    } else {
                        continue;
                    }
                }
                _ => continue,
            }
        }

        let hashlock =
            hashlock.ok_or_else(|| Error::Protocol("No hashlock provided".to_string()))?;

        let locktime =
            locktime.ok_or_else(|| Error::Protocol("No timelock provided".to_string()))?;

        let funding_addrs = Address::from_str(&reverse_response.lockup_address)?;

        let blinding_key = reverse_response
            .blinding_key
            .as_deref()
            .map(|key| ZKKeyPair::from_seckey_str(&Secp256k1::new(), key))
            .transpose()?;
        let asset_context = LiquidAssetContext::from_response(
            reverse_response.asset_id.as_deref(),
            reverse_response.fee_asset_id.as_deref(),
        )?;

        Ok(Self {
            swap_type: SwapType::ReverseSubmarine,
            side: None,
            funding_addrs: Some(funding_addrs),
            hashlock,
            receiver_pubkey: our_pubkey,
            locktime,
            sender_pubkey: reverse_response.refund_public_key,
            blinding_key,
            asset_context,
            expected_amount: reverse_response.onchain_amount,
        })
    }

    /// Create the struct for a chain swap from boltz create response.
    pub fn chain_from_swap_resp(
        side: Side,
        chain_swap_details: ChainSwapDetails,
        our_pubkey: PublicKey,
    ) -> Result<Self, Error> {
        let claim_script = Script::from_hex(&chain_swap_details.swap_tree.claim_leaf.output)?;
        let refund_script = Script::from_hex(&chain_swap_details.swap_tree.refund_leaf.output)?;

        let claim_instructions = claim_script.instructions();
        let refund_instructions = refund_script.instructions();

        let mut last_op = OP_0NOTEQUAL;
        let mut hashlock = None;
        let mut locktime = None;

        for instruction in claim_instructions {
            match instruction {
                Ok(Instruction::PushBytes(bytes)) => {
                    if bytes.len() == 20 {
                        hashlock = Some(hash160::Hash::from_slice(bytes)?);
                    } else {
                        continue;
                    }
                }
                _ => continue,
            }
        }

        for instruction in refund_instructions {
            match instruction {
                Ok(Instruction::Op(opcode)) => last_op = opcode,
                Ok(Instruction::PushBytes(bytes)) => {
                    if last_op == OP_CHECKSIGVERIFY {
                        locktime =
                            Some(LockTime::from_consensus(bytes_to_u32_little_endian(bytes)));
                    } else {
                        continue;
                    }
                }
                _ => continue,
            }
        }

        let hashlock =
            hashlock.ok_or_else(|| Error::Protocol("No hashlock provided".to_string()))?;

        let locktime =
            locktime.ok_or_else(|| Error::Protocol("No timelock provided".to_string()))?;

        let funding_addrs = Address::from_str(&chain_swap_details.lockup_address)?;

        let (sender_pubkey, receiver_pubkey) = match side {
            Side::Lockup => (our_pubkey, chain_swap_details.server_public_key),
            Side::Claim => (chain_swap_details.server_public_key, our_pubkey),
        };

        let blinding_key = chain_swap_details
            .blinding_key
            .as_deref()
            .map(|key| ZKKeyPair::from_seckey_str(&Secp256k1::new(), key))
            .transpose()?;
        let asset_context = LiquidAssetContext::from_response(
            chain_swap_details.asset_id.as_deref(),
            chain_swap_details.fee_asset_id.as_deref(),
        )?;

        Ok(Self {
            swap_type: SwapType::Chain,
            side: Some(side),
            funding_addrs: Some(funding_addrs),
            hashlock,
            receiver_pubkey,
            locktime,
            sender_pubkey,
            blinding_key,
            asset_context,
            expected_amount: chain_swap_details.amount,
        })
    }

    fn claim_script(&self) -> Script {
        match self.swap_type {
            SwapType::Submarine => EBuilder::new()
                .push_opcode(OP_HASH160)
                .push_slice(self.hashlock.as_byte_array())
                .push_opcode(OP_EQUALVERIFY)
                .push_slice(&self.receiver_pubkey.inner.x_only_public_key().0.serialize())
                .push_opcode(OP_CHECKSIG)
                .into_script(),

            SwapType::ReverseSubmarine | SwapType::Chain => EBuilder::new()
                .push_opcode(OP_SIZE)
                .push_int(32)
                .push_opcode(OP_EQUALVERIFY)
                .push_opcode(OP_HASH160)
                .push_slice(self.hashlock.as_byte_array())
                .push_opcode(OP_EQUALVERIFY)
                .push_slice(&self.receiver_pubkey.inner.x_only_public_key().0.serialize())
                .push_opcode(OP_CHECKSIG)
                .into_script(),
        }
    }

    fn refund_script(&self) -> Script {
        // Refund scripts are same for all swap types
        EBuilder::new()
            .push_slice(&self.sender_pubkey.inner.x_only_public_key().0.serialize())
            .push_opcode(OP_CHECKSIGVERIFY)
            .push_int(self.locktime.to_consensus_u32().into())
            .push_opcode(OP_CLTV)
            .into_script()
    }

    pub fn musig_keyagg_cache(&self) -> musig::KeyAggCache {
        match (self.swap_type, self.side.clone()) {
            (SwapType::ReverseSubmarine, _) | (SwapType::Chain, Some(Side::Claim)) => {
                let pubkeys = [self.sender_pubkey.inner, self.receiver_pubkey.inner];
                let [a, b] = convert_pubkeys_for_musig(&pubkeys);
                musig::KeyAggCache::new(&[&a, &b])
            }

            (SwapType::Submarine, _) | (SwapType::Chain, _) => {
                let pubkeys = [self.receiver_pubkey.inner, self.sender_pubkey.inner];
                let [a, b] = convert_pubkeys_for_musig(&pubkeys);
                musig::KeyAggCache::new(&[&a, &b])
            }
        }
    }

    /// Internally used to convert struct into a bitcoin::Script type
    fn taproot_spendinfo(&self) -> Result<TaprootSpendInfo, Error> {
        let secp = Secp256k1::new();

        // Setup Key Aggregation cache
        let key_agg_cache = self.musig_keyagg_cache();

        // Construct the Taproot
        let internal_key = key_agg_cache.agg_pk();

        let taproot_builder = TaprootBuilder::new();

        let taproot_builder =
            taproot_builder.add_leaf_with_ver(1, self.claim_script(), LeafVersion::default())?;
        let taproot_builder =
            taproot_builder.add_leaf_with_ver(1, self.refund_script(), LeafVersion::default())?;

        let taproot_spend_info =
            taproot_builder.finalize(&secp, convert_xonly_key(internal_key))?;

        // Verify taproot construction
        if let Some(funding_addrs) = &self.funding_addrs {
            let claim_key = taproot_spend_info.output_key();

            let lockup_spk = funding_addrs.script_pubkey();

            let pubkey_instruction = lockup_spk
                .instructions()
                .last()
                .ok_or(Error::Protocol(
                    "Script should contain at least one instruction".to_string(),
                ))?
                .map_err(|_| Error::Protocol("Failed to parse script instruction".to_string()))?;

            let lockup_xonly_pubkey_bytes = pubkey_instruction.push_bytes().ok_or(
                Error::Protocol("Expected push bytes instruction for pubkey".to_string()),
            )?;

            let lockup_xonly_pubkey = XOnlyPublicKey::from_slice(lockup_xonly_pubkey_bytes)?;

            if lockup_xonly_pubkey != claim_key.into_inner() {
                return Err(Error::Protocol(format!(
                    "Taproot construction Failed. Lockup Pubkey: {lockup_xonly_pubkey}, Claim Pubkey {claim_key:?}"
                )));
            }

            log::info!("Taproot creation and verification success!");
        }

        Ok(taproot_spend_info)
    }

    pub fn resolved_asset_context(&self, network: LiquidChain) -> LiquidAssetContext {
        self.asset_context
            .unwrap_or_else(|| LiquidAssetContext::legacy_lbtc(network))
    }

    pub fn requires_caller_funded_pset(&self) -> bool {
        self.asset_context
            .is_some_and(|context| !context.is_policy_asset_swap())
    }

    pub(crate) fn validate_currency(
        &self,
        network: LiquidChain,
        currency: Currency,
    ) -> Result<(), Error> {
        let context = self.resolved_asset_context(network);
        match currency {
            Currency::LBtc
                if context.swap_asset == network.bitcoin()
                    && context.policy_asset == network.bitcoin() =>
            {
                Ok(())
            }
            Currency::LUsdt
                if self.asset_context.is_some()
                    && context.swap_asset != context.policy_asset
                    && context.policy_asset == network.bitcoin() =>
            {
                Ok(())
            }
            Currency::Btc => Err(Error::Protocol(
                "BTC is not a valid Liquid swap currency".to_string(),
            )),
            _ => Err(Error::Protocol(format!(
                "Liquid response assets do not match requested currency {currency}"
            ))),
        }
    }

    /// Get the taproot address for the swap script. The address is
    /// confidential only when the response supplied a blinding key.
    pub fn to_address(&self, network: LiquidChain) -> Result<EAddress, Error> {
        let taproot_spend_info = self.taproot_spendinfo()?;

        Ok(EAddress::p2tr(
            &Secp256k1::new(),
            taproot_spend_info.internal_key(),
            taproot_spend_info.merkle_root(),
            self.blinding_key.as_ref().map(ZKKeyPair::public_key),
            network.into(),
        ))
    }

    pub fn validate_address(&self, chain: LiquidChain, address: String) -> Result<(), Error> {
        let provided = Address::parse_with_params(&address, chain.into())?;
        match (
            provided.blinding_pubkey.is_some(),
            self.blinding_key.is_some(),
        ) {
            (true, false) => {
                return Err(Error::Protocol(
                    "Confidential Liquid address requires a blinding key".to_string(),
                ));
            }
            (false, true) => {
                return Err(Error::Protocol(
                    "Explicit Liquid address must not include a blinding key".to_string(),
                ));
            }
            _ => {}
        }

        let to_address = self.to_address(chain)?;
        if to_address == provided {
            Ok(())
        } else {
            Err(Error::Protocol("Script/LockupAddress Mismatch".to_string()))
        }
    }

    pub(crate) fn blinding_secret(&self) -> Option<SecretKey> {
        self.blinding_key.as_ref().map(ZKKeyPair::secret_key)
    }

    fn required_blinding_secret(&self) -> Result<SecretKey, Error> {
        self.blinding_secret().ok_or_else(|| {
            Error::Protocol(
                "Legacy single-input Liquid transactions require a confidential L-BTC HTLC"
                    .to_string(),
            )
        })
    }

    fn select_utxo(
        &self,
        candidates: impl IntoIterator<Item = (OutPoint, TxOut)>,
        network: LiquidChain,
        expected_txid: Option<elements::Txid>,
    ) -> Result<Option<(OutPoint, TxOut)>, Error> {
        let address = self.to_address(network)?;
        let context = self.resolved_asset_context(network);
        let mut first_validation_error = None;

        for (outpoint, output) in candidates {
            if output.script_pubkey != address.script_pubkey()
                || expected_txid.is_some_and(|txid| outpoint.txid != txid)
            {
                continue;
            }

            match decode_swap_output(&output, self.blinding_secret(), context.swap_asset) {
                Ok(secrets) if secrets.value == self.expected_amount => {
                    return Ok(Some((outpoint, output)));
                }
                Ok(secrets) => {
                    first_validation_error.get_or_insert_with(|| {
                        Error::Protocol(format!(
                            "Liquid swap amount mismatch: expected {}, got {}",
                            self.expected_amount, secrets.value
                        ))
                    });
                }
                Err(error) => {
                    first_validation_error.get_or_insert(error);
                }
            }
        }

        if let Some(error) = first_validation_error {
            Err(error)
        } else {
            Ok(None)
        }
    }

    /// Fetch utxo for script from Electrum
    pub async fn fetch_utxo<LC: LiquidClient + ?Sized>(
        &self,
        liquid_client: &LC,
    ) -> Result<Option<(OutPoint, TxOut)>, Error> {
        let address = self.to_address(liquid_client.network())?;
        let candidates = liquid_client.get_address_utxos(&address).await?;
        self.select_utxo(candidates, liquid_client.network(), None)
    }

    pub(crate) async fn fetch_swap_utxo<LC: LiquidClient + ?Sized>(
        &self,
        lockup_tx: Option<&Transaction>,
        liquid_client: &LC,
        kaleidoswap_sdk: &BoltzApiClientV2,
        swap_id: &str,
        tx_kind: SwapTxKind,
    ) -> Result<(OutPoint, TxOut), Error> {
        let utxo = match lockup_tx {
            Some(tx) => self.find_utxo(tx, liquid_client.network()).await,
            None => match self.fetch_utxo(liquid_client).await {
                Ok(Some(r)) => Ok(r),
                Ok(None) | Err(_) => {
                    self.fetch_lockup_utxo_boltz(
                        liquid_client.network(),
                        kaleidoswap_sdk,
                        swap_id,
                        tx_kind,
                    )
                    .await
                }
            },
        }?;
        Ok(utxo)
    }

    pub(crate) async fn find_utxo(
        &self,
        tx: &Transaction,
        network: LiquidChain,
    ) -> Result<(OutPoint, TxOut), Error> {
        let candidates = tx
            .output
            .iter()
            .cloned()
            .enumerate()
            .map(|(vout, output)| (OutPoint::new(tx.txid(), vout as u32), output));
        self.select_utxo(candidates, network, Some(tx.txid()))?
            .ok_or(Error::Protocol(
                "No Liquid UTXO matched script, asset, amount, and transaction".to_string(),
            ))
    }

    /// Fetch utxo for script from BoltzApi
    pub async fn fetch_lockup_utxo_boltz(
        &self,
        network: LiquidChain,
        kaleidoswap_sdk: &BoltzApiClientV2,
        swap_id: &str,
        tx_kind: SwapTxKind,
    ) -> Result<(OutPoint, TxOut), Error> {
        let hex = match self.swap_type {
            SwapType::Chain => match tx_kind {
                SwapTxKind::Claim => {
                    kaleidoswap_sdk
                        .get_chain_txs(swap_id)
                        .await?
                        .server_lock
                        .ok_or(Error::Protocol(
                            "No server_lock transaction for Chain Swap available".to_string(),
                        ))?
                        .transaction
                        .hex
                }
                SwapTxKind::Refund => {
                    kaleidoswap_sdk
                        .get_chain_txs(swap_id)
                        .await?
                        .user_lock
                        .ok_or(Error::Protocol(
                            "No user_lock transaction for Chain Swap available".to_string(),
                        ))?
                        .transaction
                        .hex
                }
            },
            SwapType::ReverseSubmarine => kaleidoswap_sdk.get_reverse_tx(swap_id).await?.hex,
            SwapType::Submarine => kaleidoswap_sdk.get_submarine_tx(swap_id).await?.hex,
        };
        if hex.is_none() {
            return Err(Error::Hex(
                "No transaction hex found in boltz response".to_string(),
            ));
        }
        let tx: Transaction = elements::encode::deserialize(&hex::decode(hex.unwrap())?)?;
        self.find_utxo(&tx, network).await
    }

    // Get the chain genesis hash. Requires for sighash calculation
    pub async fn genesis_hash<LC: LiquidClient>(
        &self,
        liquid_client: &LC,
    ) -> Result<BlockHash, Error> {
        liquid_client.get_genesis_hash().await
    }
}

fn bytes_to_u32_little_endian(bytes: &[u8]) -> u32 {
    let mut result = 0u32;
    for (i, &byte) in bytes.iter().enumerate() {
        result |= (byte as u32) << (8 * i);
    }
    result
}

/// Immutable swap intent used to validate and finalize a caller-funded PSET.
///
/// The wallet never gets authority to change the swap outpoint, payment script,
/// swap amount, asset ids, timeout mode, genesis hash, or effective fee cap.
#[derive(Debug, Clone)]
pub struct PreparedLiquidSpend {
    kind: SwapTxKind,
    swap_script: LiquidSwapScript,
    funding_outpoint: OutPoint,
    funding_utxo: TxOut,
    payment_script: Script,
    genesis_hash: BlockHash,
    asset_context: LiquidAssetContext,
    amount: u64,
    max_fee: u64,
    template: LiquidPsetTemplate,
}

impl PreparedLiquidSpend {
    pub fn new(
        kind: SwapTxKind,
        swap_script: LiquidSwapScript,
        output_address: &str,
        funding_outpoint: OutPoint,
        funding_utxo: TxOut,
        genesis_hash: BlockHash,
        max_fee: u64,
    ) -> Result<Self, Error> {
        if !swap_script.requires_caller_funded_pset() {
            return Err(Error::Protocol(
                "Caller-funded PSET spends are only available for non-policy Liquid assets"
                    .to_string(),
            ));
        }
        match kind {
            SwapTxKind::Claim if swap_script.swap_type == SwapType::Submarine => {
                return Err(Error::Protocol(
                    "Claim transactions cannot be constructed for Submarine swaps.".to_string(),
                ));
            }
            SwapTxKind::Refund if swap_script.swap_type == SwapType::ReverseSubmarine => {
                return Err(Error::Protocol(
                    "Refund transactions cannot be constructed for Reverse swaps.".to_string(),
                ));
            }
            _ => {}
        }

        let asset_context = swap_script.asset_context.ok_or_else(|| {
            Error::Protocol("Caller-funded PSET requires explicit Liquid asset ids".to_string())
        })?;
        let secrets = decode_swap_output(
            &funding_utxo,
            swap_script.blinding_secret(),
            asset_context.swap_asset,
        )?;
        if swap_script
            .funding_addrs
            .as_ref()
            .is_some_and(|address| address.script_pubkey() != funding_utxo.script_pubkey)
        {
            return Err(Error::Protocol(
                "Liquid swap witness_utxo does not pay the expected swap script".to_string(),
            ));
        }
        if secrets.value != swap_script.expected_amount {
            return Err(Error::Protocol(format!(
                "Liquid swap amount mismatch: expected {}, got {}",
                swap_script.expected_amount, secrets.value
            )));
        }
        let output_address = Address::from_str(output_address)?;
        let payment_script = output_address.script_pubkey();
        if max_fee == 0 {
            return Err(Error::Protocol(
                "Liquid fee cap must be greater than zero".to_string(),
            ));
        }

        let lock_time = match kind {
            SwapTxKind::Claim => LockTime::ZERO,
            SwapTxKind::Refund => swap_script.locktime,
        };
        let unsigned = Transaction {
            version: 2,
            lock_time,
            input: vec![TxIn {
                previous_output: funding_outpoint,
                is_pegin: false,
                script_sig: Script::new(),
                sequence: Sequence::ZERO,
                asset_issuance: AssetIssuance::default(),
                witness: TxInWitness::default(),
            }],
            output: vec![TxOut {
                asset: Asset::Explicit(asset_context.swap_asset),
                value: Value::Explicit(secrets.value),
                nonce: output_address
                    .blinding_pubkey
                    .map(elements::confidential::Nonce::from)
                    .unwrap_or_default(),
                script_pubkey: payment_script.clone(),
                witness: TxOutWitness::default(),
            }],
        };
        let mut pset = PartiallySignedTransaction::from_tx(unsigned);
        let swap_input = &mut pset.inputs_mut()[0];
        swap_input.witness_utxo = Some(funding_utxo.clone());
        swap_input.asset = Some(asset_context.swap_asset);
        swap_input.amount = Some(secrets.value);

        let template = LiquidPsetTemplate {
            pset: pset.to_string(),
            swap_input_index: 0,
            payment_output_index: 0,
            swap_asset_id: asset_context.swap_asset.to_string(),
            policy_asset_id: asset_context.policy_asset.to_string(),
            amount: secrets.value,
            max_fee,
        };

        Ok(Self {
            kind,
            swap_script,
            funding_outpoint,
            funding_utxo,
            payment_script,
            genesis_hash,
            asset_context,
            amount: secrets.value,
            max_fee,
            template,
        })
    }

    pub fn template(&self) -> LiquidPsetTemplate {
        self.template.clone()
    }

    pub fn finalize_claim(
        &self,
        funded: FundedLiquidPset,
        keys: &Keypair,
        preimage: &Preimage,
    ) -> Result<Transaction, Error> {
        if self.kind != SwapTxKind::Claim {
            return Err(Error::Protocol(
                "Cannot finalize a claim from a refund spend".to_string(),
            ));
        }
        let preimage = preimage.bytes.ok_or_else(|| {
            Error::Protocol(
                "Preimage bytes not available - cannot claim without actual preimage".to_string(),
            )
        })?;
        if hash160::Hash::hash(&preimage) != self.swap_script.hashlock {
            return Err(Error::Protocol(
                "Claim preimage does not match the swap hashlock".to_string(),
            ));
        }
        self.finalize(funded, keys, Some(preimage.to_vec()))
    }

    pub fn finalize_refund(
        &self,
        funded: FundedLiquidPset,
        keys: &Keypair,
    ) -> Result<Transaction, Error> {
        if self.kind != SwapTxKind::Refund {
            return Err(Error::Protocol(
                "Cannot finalize a refund from a claim spend".to_string(),
            ));
        }
        self.finalize(funded, keys, None)
    }

    fn finalize(
        &self,
        funded: FundedLiquidPset,
        keys: &Keypair,
        preimage: Option<Vec<u8>>,
    ) -> Result<Transaction, Error> {
        let expected_pubkey = match self.kind {
            SwapTxKind::Claim => self.swap_script.receiver_pubkey.inner,
            SwapTxKind::Refund => self.swap_script.sender_pubkey.inner,
        };
        if keys.public_key().serialize() != expected_pubkey.serialize() {
            return Err(Error::Protocol(
                "Liquid finalization key does not match the swap spend path".to_string(),
            ));
        }
        let pset = PartiallySignedTransaction::from_str(&funded.pset)
            .map_err(|e| Error::Protocol(format!("Invalid funded Liquid PSET: {e}")))?;
        let (mut tx, swap_input_index, prevouts) =
            self.validate_funded_pset(&pset, &funded.payment_output_secrets)?;
        let frozen_tx = tx.clone();

        let spend_script = match self.kind {
            SwapTxKind::Claim => self.swap_script.claim_script(),
            SwapTxKind::Refund => self.swap_script.refund_script(),
        };
        let leaf_hash = TapLeafHash::from_script(&spend_script, LeafVersion::default());
        let prevout_refs = prevouts.iter().collect::<Vec<_>>();
        let sighash = SighashCache::new(&tx).taproot_script_spend_signature_hash(
            swap_input_index,
            &Prevouts::All(&prevout_refs),
            leaf_hash,
            SchnorrSighashType::Default,
            self.genesis_hash,
        )?;
        let msg = Message::from_digest_slice(sighash.as_byte_array())?;
        let final_sig = SchnorrSig {
            sig: Secp256k1::new().sign_schnorr(&msg, keys),
            hash_ty: SchnorrSighashType::Default,
        };
        let control_block = self
            .swap_script
            .taproot_spendinfo()?
            .control_block(&(spend_script.clone(), LeafVersion::default()))
            .ok_or_else(|| Error::Taproot("Could not create control block".to_string()))?;
        let mut witness = Witness::new();
        witness.push(final_sig.to_vec());
        if let Some(preimage) = preimage {
            witness.push(preimage);
        }
        witness.push(spend_script.as_bytes());
        witness.push(control_block.serialize());
        tx.input[swap_input_index].witness.script_witness = witness.to_vec();

        let mut expected = tx.clone();
        expected.input[swap_input_index].witness =
            frozen_tx.input[swap_input_index].witness.clone();
        if expected != frozen_tx {
            return Err(Error::Protocol(
                "Liquid finalization attempted to mutate the unsigned transaction".to_string(),
            ));
        }
        Ok(tx)
    }

    fn validate_funded_pset(
        &self,
        pset: &PartiallySignedTransaction,
        payment_secrets: &LiquidOutputSecrets,
    ) -> Result<(Transaction, usize, Vec<TxOut>), Error> {
        pset.sanity_check()
            .map_err(|e| Error::Protocol(format!("Invalid funded Liquid PSET: {e}")))?;
        let tx = pset
            .extract_tx()
            .map_err(|e| Error::Protocol(format!("Cannot extract funded Liquid PSET: {e}")))?;
        if tx.version != 2 {
            return Err(Error::Protocol(
                "Funded Liquid PSET must use transaction version 2".to_string(),
            ));
        }
        let expected_locktime = match self.kind {
            SwapTxKind::Claim => LockTime::ZERO,
            SwapTxKind::Refund => self.swap_script.locktime,
        };
        if tx.lock_time != expected_locktime {
            return Err(Error::Protocol(
                "Funded Liquid PSET changed the swap locktime".to_string(),
            ));
        }

        let secp = Secp256k1::new();
        let mut seen = HashSet::new();
        let mut swap_input_index = None;
        let mut policy_input_total = 0u64;
        let mut prevouts = Vec::with_capacity(pset.inputs().len());
        for (index, input) in pset.inputs().iter().enumerate() {
            let outpoint = OutPoint::new(input.previous_txid, input.previous_output_index);
            if !seen.insert(outpoint) {
                return Err(Error::Protocol(
                    "Funded Liquid PSET contains duplicate inputs".to_string(),
                ));
            }
            if input.is_pegin() || input.has_issuance() {
                return Err(Error::Protocol(
                    "Liquid peg-ins and issuance are not allowed in swap PSETs".to_string(),
                ));
            }
            if input.issuance_value_rangeproof.is_some()
                || input.issuance_keys_rangeproof.is_some()
                || input.pegin_tx.is_some()
                || input.pegin_txout_proof.is_some()
                || input.pegin_genesis_hash.is_some()
                || input.pegin_claim_script.is_some()
                || input.pegin_value.is_some()
                || input.pegin_witness.is_some()
            {
                return Err(Error::Protocol(
                    "Liquid peg-in and issuance metadata is not allowed in swap PSETs".to_string(),
                ));
            }
            let witness_utxo = input.witness_utxo.clone().ok_or_else(|| {
                Error::Protocol(format!("Liquid PSET input {index} is missing witness_utxo"))
            })?;
            prevouts.push(witness_utxo.clone());

            if outpoint == self.funding_outpoint {
                if swap_input_index.replace(index).is_some() {
                    return Err(Error::Protocol(
                        "Funded Liquid PSET contains the swap input more than once".to_string(),
                    ));
                }
                if witness_utxo != self.funding_utxo
                    || input.asset != Some(self.asset_context.swap_asset)
                    || input.amount != Some(self.amount)
                    || input.sequence != Some(Sequence::ZERO)
                    || input
                        .final_script_sig
                        .as_ref()
                        .is_some_and(|script| !script.is_empty())
                    || input
                        .final_script_witness
                        .as_ref()
                        .is_some_and(|witness| !witness.is_empty())
                {
                    return Err(Error::Protocol(
                        "Funded Liquid PSET changed the swap input".to_string(),
                    ));
                }
            } else {
                if input
                    .final_script_witness
                    .as_ref()
                    .is_none_or(|witness| witness.is_empty())
                {
                    return Err(Error::Protocol(format!(
                        "Liquid wallet input {index} is not finalized"
                    )));
                }
                let (asset, value) = verify_input_metadata(&secp, input, &witness_utxo, index)?;
                if asset != self.asset_context.policy_asset {
                    return Err(Error::Protocol(format!(
                        "Liquid wallet input {index} is not the policy asset"
                    )));
                }
                policy_input_total = policy_input_total.checked_add(value).ok_or_else(|| {
                    Error::Protocol("Liquid policy input amount overflow".to_string())
                })?;
            }
        }
        let swap_input_index = swap_input_index.ok_or_else(|| {
            Error::Protocol("Funded Liquid PSET is missing the swap input".to_string())
        })?;
        if policy_input_total == 0 {
            return Err(Error::LiquidFeeAssetRequired);
        }

        let mut payment_index = None;
        let mut fee = None;
        let mut policy_change_total = 0u64;
        for (index, (output, txout)) in pset.outputs().iter().zip(&tx.output).enumerate() {
            if txout.script_pubkey.is_empty() {
                if fee.is_some()
                    || !matches!(txout.asset, Asset::Explicit(asset) if asset == self.asset_context.policy_asset)
                    || !matches!(txout.value, Value::Explicit(_))
                    || !txout.nonce.is_null()
                {
                    return Err(Error::Protocol(
                        "Swap PSET must contain exactly one explicit policy-asset fee output"
                            .to_string(),
                    ));
                }
                let Value::Explicit(value) = txout.value else {
                    unreachable!()
                };
                fee = Some(value);
                continue;
            }

            if txout.script_pubkey == self.payment_script {
                if payment_index.replace(index).is_some() {
                    return Err(Error::Protocol(
                        "Funded Liquid PSET contains multiple payment outputs".to_string(),
                    ));
                }
                self.verify_payment_output(output, txout, payment_secrets)?;
                continue;
            }

            let (asset, value) = verify_output_metadata(&secp, output, txout, index)?;
            if asset != self.asset_context.policy_asset {
                return Err(Error::Protocol(format!(
                    "Liquid output {index} is neither the full swap payout nor policy change"
                )));
            }
            policy_change_total = policy_change_total.checked_add(value).ok_or_else(|| {
                Error::Protocol("Liquid policy change amount overflow".to_string())
            })?;
        }
        payment_index.ok_or_else(|| {
            Error::Protocol("Funded Liquid PSET is missing the full swap payout".to_string())
        })?;
        let fee = fee.ok_or_else(|| {
            Error::Protocol(
                "Swap PSET must contain exactly one explicit policy-asset fee output".to_string(),
            )
        })?;
        if fee > self.max_fee {
            return Err(Error::Protocol(format!(
                "Liquid transaction fee {fee} exceeds pinned cap {}",
                self.max_fee
            )));
        }
        let required_policy = policy_change_total
            .checked_add(fee)
            .ok_or_else(|| Error::Protocol("Liquid policy output amount overflow".to_string()))?;
        if policy_input_total < required_policy {
            return Err(Error::LiquidFeeAssetRequired);
        }
        if policy_input_total != required_policy {
            return Err(Error::Protocol(
                "Liquid policy inputs do not equal policy change plus fee".to_string(),
            ));
        }
        tx.verify_tx_amt_proofs(&secp, &prevouts)
            .map_err(|e| Error::Protocol(format!("Liquid amount proof validation failed: {e}")))?;
        Ok((tx, swap_input_index, prevouts))
    }

    fn verify_payment_output(
        &self,
        output: &PsetOutput,
        txout: &TxOut,
        secrets: &LiquidOutputSecrets,
    ) -> Result<(), Error> {
        let asset = elements::AssetId::from_str(&secrets.asset_id)?;
        let abf = AssetBlindingFactor::from_str(&secrets.asset_blinding_factor)?;
        let vbf = ValueBlindingFactor::from_str(&secrets.value_blinding_factor)?;
        if asset != self.asset_context.swap_asset
            || secrets.value != self.amount
            || output.asset != Some(asset)
            || output.amount != Some(self.amount)
        {
            return Err(Error::Protocol(
                "Liquid payment output secrets do not describe the full swap payout".to_string(),
            ));
        }
        let secp = Secp256k1::new();
        let expected_asset = if abf == AssetBlindingFactor::zero() {
            Asset::Explicit(asset)
        } else {
            Asset::new_confidential(&secp, asset, abf)
        };
        let expected_value =
            if vbf == ValueBlindingFactor::zero() && abf == AssetBlindingFactor::zero() {
                Value::Explicit(self.amount)
            } else {
                Value::new_confidential_from_assetid(&secp, self.amount, asset, vbf, abf)
            };
        if txout.asset != expected_asset || txout.value != expected_value {
            return Err(Error::Protocol(
                "Liquid payment output secrets do not recreate its commitments".to_string(),
            ));
        }
        verify_output_metadata(
            &secp,
            output,
            txout,
            self.template.payment_output_index as usize,
        )?;
        Ok(())
    }
}

fn verify_input_metadata(
    secp: &Secp256k1<elements::secp256k1_zkp::All>,
    input: &PsetInput,
    witness_utxo: &TxOut,
    index: usize,
) -> Result<(elements::AssetId, u64), Error> {
    let asset = input.asset.ok_or_else(|| {
        Error::Protocol(format!(
            "Liquid PSET input {index} is missing its explicit asset"
        ))
    })?;
    let value = input.amount.ok_or_else(|| {
        Error::Protocol(format!(
            "Liquid PSET input {index} is missing its explicit amount"
        ))
    })?;
    match (witness_utxo.asset, witness_utxo.value) {
        (Asset::Explicit(committed_asset), Value::Explicit(committed_value))
            if committed_asset == asset && committed_value == value => {}
        (Asset::Confidential(asset_commit), Value::Confidential(value_commit)) => {
            let asset_proof = input.blind_asset_proof.as_ref().ok_or_else(|| {
                Error::Protocol(format!(
                    "Liquid PSET input {index} is missing blind asset proof"
                ))
            })?;
            let value_proof = input.blind_value_proof.as_ref().ok_or_else(|| {
                Error::Protocol(format!(
                    "Liquid PSET input {index} is missing blind value proof"
                ))
            })?;
            if !asset_proof.blind_asset_proof_verify(secp, asset, asset_commit)
                || !value_proof.blind_value_proof_verify(secp, value, asset_commit, value_commit)
            {
                return Err(Error::Protocol(format!(
                    "Liquid PSET input {index} blind proofs do not match witness_utxo"
                )));
            }
        }
        _ => {
            return Err(Error::Protocol(format!(
                "Liquid PSET input {index} has inconsistent or mixed commitments"
            )));
        }
    }
    Ok((asset, value))
}

fn verify_output_metadata(
    secp: &Secp256k1<elements::secp256k1_zkp::All>,
    output: &PsetOutput,
    txout: &TxOut,
    index: usize,
) -> Result<(elements::AssetId, u64), Error> {
    let asset = output.asset.ok_or_else(|| {
        Error::Protocol(format!(
            "Liquid PSET output {index} is missing its explicit asset"
        ))
    })?;
    let value = output.amount.ok_or_else(|| {
        Error::Protocol(format!(
            "Liquid PSET output {index} is missing its explicit amount"
        ))
    })?;
    match (txout.asset, txout.value) {
        (Asset::Explicit(committed_asset), Value::Explicit(committed_value))
            if committed_asset == asset && committed_value == value => {}
        (Asset::Confidential(asset_commit), Value::Confidential(value_commit)) => {
            let asset_proof = output.blind_asset_proof.as_ref().ok_or_else(|| {
                Error::Protocol(format!(
                    "Liquid PSET output {index} is missing blind asset proof"
                ))
            })?;
            let value_proof = output.blind_value_proof.as_ref().ok_or_else(|| {
                Error::Protocol(format!(
                    "Liquid PSET output {index} is missing blind value proof"
                ))
            })?;
            if !asset_proof.blind_asset_proof_verify(secp, asset, asset_commit)
                || !value_proof.blind_value_proof_verify(secp, value, asset_commit, value_commit)
            {
                return Err(Error::Protocol(format!(
                    "Liquid PSET output {index} blind proofs do not match commitments"
                )));
            }
        }
        _ => {
            return Err(Error::Protocol(format!(
                "Liquid PSET output {index} has inconsistent or mixed commitments"
            )));
        }
    }
    Ok((asset, value))
}

/// Liquid swap transaction helper.
#[derive(Debug, Clone)]
pub struct LiquidSwapTx {
    pub kind: SwapTxKind,
    pub swap_script: LiquidSwapScript,
    pub output_address: Address,
    pub funding_outpoint: OutPoint,
    pub funding_utxo: TxOut, // there should only ever be one outpoint in a swap
    pub genesis_hash: BlockHash, // Required to calculate sighash
}

/// Deprecated compatibility alias. Use [`LiquidSwapTx`].
#[deprecated(since = "0.4.2", note = "renamed to LiquidSwapTx")]
pub type LBtcSwapTx = LiquidSwapTx;

impl LiquidSwapTx {
    pub(crate) async fn new_claim_with_utxo<LC: LiquidClient + ?Sized>(
        swap_script: LiquidSwapScript,
        output_address: String,
        liquid_client: &LC,
        utxo: (OutPoint, TxOut),
    ) -> Result<LiquidSwapTx, Error> {
        if swap_script.requires_caller_funded_pset() {
            return Err(Error::Protocol(
                "L-USDT claims require the caller-funded PSET flow".to_string(),
            ));
        }
        if swap_script.swap_type == SwapType::Submarine {
            return Err(Error::Protocol(
                "Claim transactions cannot be constructed for Submarine swaps.".to_string(),
            ));
        }

        let genesis_hash = liquid_client.get_genesis_hash().await?;

        Ok(LiquidSwapTx {
            kind: SwapTxKind::Claim,
            swap_script,
            output_address: Address::from_str(&output_address)?,
            funding_outpoint: utxo.0,
            funding_utxo: utxo.1,
            genesis_hash,
        })
    }

    /// Craft a new ClaimTx. Only works for Reverse and Chain Swaps.
    pub async fn new_claim<LC: LiquidClient + ?Sized>(
        swap_script: LiquidSwapScript,
        output_address: String,
        liquid_client: &LC,
        kaleidoswap_sdk: &BoltzApiClientV2,
        swap_id: String,
    ) -> Result<LiquidSwapTx, Error> {
        if swap_script.requires_caller_funded_pset() {
            return Err(Error::Protocol(
                "L-USDT claims require the caller-funded PSET flow".to_string(),
            ));
        }
        let utxo = swap_script
            .fetch_swap_utxo(
                None,
                liquid_client,
                kaleidoswap_sdk,
                &swap_id,
                SwapTxKind::Claim,
            )
            .await?;

        Self::new_claim_with_utxo(swap_script, output_address, liquid_client, utxo).await
    }

    /// Construct a RefundTX corresponding to the swap_script. Only works for Submarine and Chain Swaps.
    pub async fn new_refund<LC: LiquidClient + ?Sized>(
        swap_script: LiquidSwapScript,
        output_address: &str,
        liquid_client: &LC,
        kaleidoswap_sdk: &BoltzApiClientV2,
        swap_id: String,
    ) -> Result<LiquidSwapTx, Error> {
        if swap_script.requires_caller_funded_pset() {
            return Err(Error::Protocol(
                "L-USDT refunds require the caller-funded PSET flow".to_string(),
            ));
        }
        if swap_script.swap_type == SwapType::ReverseSubmarine {
            return Err(Error::Protocol(
                "Refund Txs cannot be constructed for Reverse Submarine Swaps.".to_string(),
            ));
        }

        let address = Address::from_str(output_address)?;
        let (funding_outpoint, funding_utxo) = swap_script
            .fetch_swap_utxo(
                None,
                liquid_client,
                kaleidoswap_sdk,
                &swap_id,
                SwapTxKind::Refund,
            )
            .await?;

        let genesis_hash = liquid_client.get_genesis_hash().await?;

        Ok(LiquidSwapTx {
            kind: SwapTxKind::Refund,
            swap_script,
            output_address: address,
            funding_outpoint,
            funding_utxo,
            genesis_hash,
        })
    }

    /// Compute the Musig partial signature.
    /// This is used to cooperatively close a Submarine or Chain Swap.
    pub fn partial_sign(
        &self,
        keys: &Keypair,
        pub_nonce: &str,
        transaction_hash: &str,
    ) -> Result<(musig::PartialSignature, musig::PublicNonce), Error> {
        if self.swap_script.requires_caller_funded_pset() {
            return Err(Error::Protocol(
                "Cooperative Liquid signing is disabled for L-USDT".to_string(),
            ));
        }
        self.swap_script
            .partial_sign(keys, pub_nonce, transaction_hash)
    }

    /// Sign a claim transaction.
    /// Panics if called on a Submarine Swap or Refund Tx.
    /// If the claim is cooperative, provide the other party's partial sigs.
    /// If this is None, transaction will be claimed via taproot script path.
    pub async fn sign_claim(
        &self,
        keys: &Keypair,
        preimage: &Preimage,
        fee: Fee,
        is_cooperative: Option<Cooperative<'_>>,
        is_discount_ct: bool,
    ) -> Result<Transaction, Error> {
        if self.swap_script.requires_caller_funded_pset() {
            return Err(Error::Protocol(
                "L-USDT claims require the caller-funded PSET flow".to_string(),
            ));
        }
        if self.swap_script.swap_type == SwapType::Submarine {
            return Err(Error::Protocol(
                "Claim Tx signing is not applicable for Submarine Swaps".to_string(),
            ));
        }

        if self.kind == SwapTxKind::Refund {
            return Err(Error::Protocol(
                "Cannot sign claim with refund-type LiquidSwapTx".to_string(),
            ));
        }

        let mut claim_tx = create_tx_with_fee(
            fee,
            |fee| self.create_claim(keys, preimage, fee, is_cooperative.is_some()),
            |tx| tx_size(&tx, is_discount_ct),
        )?;

        // If its a cooperative claim, compute the Musig2 Aggregate Signature and use Keypath spending
        if let Some(Cooperative {
            boltz_api,
            swap_id,
            signature,
        }) = is_cooperative
        {
            let claim_tx_taproot_hash = SighashCache::new(&claim_tx)
                .taproot_key_spend_signature_hash(
                    0,
                    &Prevouts::All(&[&self.funding_utxo]),
                    SchnorrSighashType::Default,
                    self.genesis_hash,
                )?;

            let msg = *claim_tx_taproot_hash.as_byte_array();

            let mut key_agg_cache = self.swap_script.musig_keyagg_cache();

            let tweak = Scalar::from_be_bytes(
                *self
                    .swap_script
                    .taproot_spendinfo()?
                    .tap_tweak()
                    .as_byte_array(),
            )?;

            let _ = key_agg_cache.pubkey_xonly_tweak_add(&tweak)?;

            let session_secret_rand =
                musig::SessionSecretRand::assume_unique_per_nonce_gen(rng_32b());

            let mut extra_rand = [0u8; 32];
            OsRng.fill_bytes(&mut extra_rand);

            let (claim_sec_nonce, claim_pub_nonce) = key_agg_cache.nonce_gen(
                session_secret_rand,
                convert_public_key(keys.public_key()),
                &msg,
                Some(extra_rand),
            );

            // Step 7: Get boltz's partial sig
            let claim_tx_hex = serialize(&claim_tx).to_lower_hex_string();
            let partial_sig_resp = match self.swap_script.swap_type {
                SwapType::Chain => {
                    boltz_api
                        .post_chain_claim_tx_details(
                            &swap_id,
                            preimage,
                            signature,
                            ToSign {
                                pub_nonce: claim_pub_nonce.serialize().to_lower_hex_string(),
                                transaction: claim_tx_hex,
                                index: 0,
                            },
                        )
                        .await
                }
                SwapType::ReverseSubmarine => {
                    boltz_api
                        .get_reverse_partial_sig(
                            &swap_id,
                            preimage,
                            &claim_pub_nonce,
                            &claim_tx_hex,
                        )
                        .await
                }
                _ => Err(Error::Protocol(format!(
                    "Cannot get partial sig for {:?} Swap",
                    self.swap_script.swap_type
                ))),
            }?;

            let boltz_public_nonce = musig::PublicNonce::from_str(&partial_sig_resp.pub_nonce)?;

            let boltz_partial_sig =
                musig::PartialSignature::from_str(&partial_sig_resp.partial_signature)?;

            let agg_nonce = musig::AggregatedNonce::new(&[&boltz_public_nonce, &claim_pub_nonce]);

            let musig_session = musig::Session::new(&key_agg_cache, agg_nonce, &msg);

            // Verify the sigs.
            let boltz_partial_sig_verify = musig_session.partial_verify(
                &key_agg_cache,
                &boltz_partial_sig,
                &boltz_public_nonce,
                convert_public_key(self.swap_script.sender_pubkey.inner), //boltz key
            );

            if !boltz_partial_sig_verify {
                return Err(Error::Taproot(
                    "Unable to verify Partial Signature".to_string(),
                ));
            }

            let our_partial_sig =
                musig_session.partial_sign(claim_sec_nonce, &convert_keypair(keys), &key_agg_cache);

            let schnorr_sig = musig_session
                .partial_sig_agg(&[&boltz_partial_sig, &our_partial_sig])
                .assume_valid();

            let final_schnorr_sig = SchnorrSig {
                sig: convert_schnorr_signature(schnorr_sig),
                hash_ty: SchnorrSighashType::Default,
            };

            let output_key = self.swap_script.taproot_spendinfo()?.output_key();

            let secp = Secp256k1::new();
            let msg = Message::from_digest_slice(&msg)?;
            secp.verify_schnorr(&final_schnorr_sig.sig, &msg, &output_key.into_inner())?;

            let mut script_witness = Witness::new();
            script_witness.push(final_schnorr_sig.to_vec());

            let witness = TxInWitness {
                amount_rangeproof: None,
                inflation_keys_rangeproof: None,
                script_witness: script_witness.to_vec(),
                pegin_witness: vec![],
            };

            claim_tx.input[0].witness = witness;
        }

        Ok(claim_tx)
    }

    fn create_claim(
        &self,
        keys: &Keypair,
        preimage: &Preimage,
        absolute_fees: u64,
        is_cooperative: bool,
    ) -> Result<Transaction, Error> {
        if preimage.bytes.is_none() {
            return Err(Error::Protocol("No preimage provided".to_string()));
        }

        let claim_txin = TxIn {
            sequence: Sequence::MAX,
            previous_output: self.funding_outpoint,
            script_sig: Script::new(),
            witness: TxInWitness::default(),
            is_pegin: false,
            asset_issuance: AssetIssuance::default(),
        };

        let secp = Secp256k1::new();
        let mut rng = OsRng;

        let unblined_utxo = self
            .funding_utxo
            .unblind(&secp, self.swap_script.required_blinding_secret()?)?;
        let asset_id = unblined_utxo.asset;
        let out_abf = AssetBlindingFactor::new(&mut rng);
        let exp_asset = Asset::Explicit(asset_id);

        let (blinded_asset, asset_surjection_proof) =
            exp_asset.blind(&mut rng, &secp, out_abf, &[unblined_utxo])?;

        let output_value = Amount::from_sat(unblined_utxo.value)
            .checked_sub(Amount::from_sat(absolute_fees))
            .ok_or(Error::Protocol(format!(
                "Output value {} is less than fees {}",
                unblined_utxo.value, absolute_fees
            )))?;

        let final_vbf = ValueBlindingFactor::last(
            &secp,
            output_value.to_sat(),
            out_abf,
            &[(
                unblined_utxo.value,
                unblined_utxo.asset_bf,
                unblined_utxo.value_bf,
            )],
            &[(
                absolute_fees,
                AssetBlindingFactor::zero(),
                ValueBlindingFactor::zero(),
            )],
        );
        let explicit_value = elements::confidential::Value::Explicit(output_value.to_sat());
        let msg = elements::RangeProofMessage {
            asset: asset_id,
            bf: out_abf,
        };
        let ephemeral_sk = SecretKey::new(&mut rng);

        // assuming we always use a blinded address that has an extractable blinding pub
        let blinding_key = self
            .output_address
            .blinding_pubkey
            .ok_or(Error::Protocol("No blinding key in tx.".to_string()))?;
        let (blinded_value, nonce, rangeproof) = explicit_value.blind(
            &secp,
            final_vbf,
            blinding_key,
            ephemeral_sk,
            &self.output_address.script_pubkey(),
            &msg,
        )?;

        let tx_out_witness = TxOutWitness {
            surjection_proof: Some(Box::new(asset_surjection_proof)), // from asset blinding
            rangeproof: Some(Box::new(rangeproof)),                   // from value blinding
        };
        let payment_output: TxOut = TxOut {
            script_pubkey: self.output_address.script_pubkey(),
            value: blinded_value,
            asset: blinded_asset,
            nonce,
            witness: tx_out_witness,
        };
        let fee_output: TxOut = TxOut::new_fee(absolute_fees, asset_id);

        let mut claim_tx = Transaction {
            version: 2,
            lock_time: LockTime::ZERO,
            input: vec![claim_txin],
            output: vec![payment_output, fee_output],
        };

        if is_cooperative {
            claim_tx.input[0].witness = Self::stubbed_cooperative_witness();
        } else {
            // If Non-Cooperative claim use the Script Path spending
            claim_tx.input[0].sequence = Sequence::ZERO;
            let claim_script = self.swap_script.claim_script();
            let leaf_hash = TapLeafHash::from_script(&claim_script, LeafVersion::default());

            let sighash = SighashCache::new(&claim_tx).taproot_script_spend_signature_hash(
                0,
                &Prevouts::All(&[&self.funding_utxo]),
                leaf_hash,
                SchnorrSighashType::Default,
                self.genesis_hash,
            )?;

            let msg = Message::from_digest_slice(sighash.as_byte_array())?;

            let sig = secp.sign_schnorr(&msg, keys);

            let final_sig = SchnorrSig {
                sig,
                hash_ty: SchnorrSighashType::Default,
            };

            let control_block = match self
                .swap_script
                .taproot_spendinfo()?
                .control_block(&(claim_script.clone(), LeafVersion::default()))
            {
                Some(r) => r,
                None => return Err(Error::Taproot("Could not create control block".to_string())),
            };

            let mut script_witness = Witness::new();
            script_witness.push(final_sig.to_vec());
            script_witness.push(preimage.bytes.ok_or(Error::Protocol(
                "Preimage bytes not available - cannot claim without actual preimage".to_string(),
            ))?);
            script_witness.push(claim_script.as_bytes());
            script_witness.push(control_block.serialize());

            let witness = TxInWitness {
                amount_rangeproof: None,
                inflation_keys_rangeproof: None,
                script_witness: script_witness.to_vec(),
                pegin_witness: vec![],
            };

            claim_tx.input[0].witness = witness;
        }

        Ok(claim_tx)
    }

    /// Sign a refund transaction.
    /// Panics if called on a Reverse Swap or Claim Tx.
    pub async fn sign_refund(
        &self,
        keys: &Keypair,
        fee: Fee,
        is_cooperative: Option<Cooperative<'_>>,
        is_discount_ct: bool,
    ) -> Result<Transaction, Error> {
        if self.swap_script.requires_caller_funded_pset() {
            return Err(Error::Protocol(
                "L-USDT refunds require the caller-funded PSET flow".to_string(),
            ));
        }
        if self.swap_script.swap_type == SwapType::ReverseSubmarine {
            return Err(Error::Protocol(
                "Refund Tx signing is not applicable for Reverse Submarine Swaps".to_string(),
            ));
        }

        if self.kind == SwapTxKind::Claim {
            return Err(Error::Protocol(
                "Cannot sign refund with a claim-type LiquidSwapTx".to_string(),
            ));
        }

        let mut refund_tx = create_tx_with_fee(
            fee,
            |fee| self.create_refund(keys, fee, is_cooperative.is_some()),
            |tx| tx_size(&tx, is_discount_ct),
        )?;

        if let Some(Cooperative {
            boltz_api, swap_id, ..
        }) = is_cooperative
        {
            let secp = Secp256k1::new();

            refund_tx.lock_time = LockTime::ZERO;

            let claim_tx_taproot_hash = SighashCache::new(&refund_tx)
                .taproot_key_spend_signature_hash(
                    0,
                    &Prevouts::All(&[&self.funding_utxo]),
                    SchnorrSighashType::Default,
                    self.genesis_hash,
                )?;

            let msg = *claim_tx_taproot_hash.as_byte_array();

            let mut key_agg_cache = self.swap_script.musig_keyagg_cache();

            let tweak = Scalar::from_be_bytes(
                *self
                    .swap_script
                    .taproot_spendinfo()?
                    .tap_tweak()
                    .as_byte_array(),
            )?;

            let _ = key_agg_cache.pubkey_xonly_tweak_add(&tweak)?;

            let session_secret_rand =
                musig::SessionSecretRand::assume_unique_per_nonce_gen(rng_32b());

            let mut extra_rand = [0u8; 32];
            OsRng.fill_bytes(&mut extra_rand);

            let (sec_nonce, pub_nonce) = key_agg_cache.nonce_gen(
                session_secret_rand,
                convert_public_key(keys.public_key()),
                &msg,
                Some(extra_rand),
            );

            // Step 7: Get boltz's partial sig
            let refund_tx_hex = serialize(&refund_tx).to_lower_hex_string();
            let partial_sig_resp = match self.swap_script.swap_type {
                SwapType::Chain => {
                    boltz_api
                        .get_chain_partial_sig(&swap_id, 0, &pub_nonce, &refund_tx_hex)
                        .await
                }
                SwapType::Submarine => {
                    boltz_api
                        .get_submarine_partial_sig(&swap_id, 0, &pub_nonce, &refund_tx_hex)
                        .await
                }
                _ => Err(Error::Protocol(format!(
                    "Cannot get partial sig for {:?} Swap",
                    self.swap_script.swap_type
                ))),
            }?;

            let boltz_public_nonce = musig::PublicNonce::from_str(&partial_sig_resp.pub_nonce)?;

            let boltz_partial_sig =
                musig::PartialSignature::from_str(&partial_sig_resp.partial_signature)?;

            let agg_nonce = musig::AggregatedNonce::new(&[&boltz_public_nonce, &pub_nonce]);

            let musig_session = musig::Session::new(&key_agg_cache, agg_nonce, &msg);

            // Verify the sigs.
            let boltz_partial_sig_verify = musig_session.partial_verify(
                &key_agg_cache,
                &boltz_partial_sig,
                &boltz_public_nonce,
                convert_public_key(self.swap_script.receiver_pubkey.inner), //boltz key
            );

            if !boltz_partial_sig_verify {
                return Err(Error::Taproot(
                    "Unable to verify Partial Signature".to_string(),
                ));
            }

            let our_partial_sig =
                musig_session.partial_sign(sec_nonce, &convert_keypair(keys), &key_agg_cache);

            let schnorr_sig = musig_session
                .partial_sig_agg(&[&boltz_partial_sig, &our_partial_sig])
                .assume_valid();

            let final_schnorr_sig = SchnorrSig {
                sig: convert_schnorr_signature(schnorr_sig),
                hash_ty: SchnorrSighashType::Default,
            };

            let output_key = self.swap_script.taproot_spendinfo()?.output_key();

            let msg = Message::from_digest_slice(&msg)?;
            secp.verify_schnorr(&final_schnorr_sig.sig, &msg, &output_key.into_inner())?;

            let mut script_witness = Witness::new();
            script_witness.push(final_schnorr_sig.to_vec());

            let witness = TxInWitness {
                amount_rangeproof: None,
                inflation_keys_rangeproof: None,
                script_witness: script_witness.to_vec(),
                pegin_witness: vec![],
            };

            refund_tx.input[0].witness = witness;
        }

        Ok(refund_tx)
    }

    fn create_refund(
        &self,
        keys: &Keypair,
        absolute_fees: u64,
        is_cooperative: bool,
    ) -> Result<Transaction, Error> {
        // Create unsigned refund transaction
        let refund_txin = TxIn {
            sequence: Sequence::MAX,
            previous_output: self.funding_outpoint,
            script_sig: Script::new(),
            witness: TxInWitness::default(),
            is_pegin: false,
            asset_issuance: AssetIssuance::default(),
        };

        let secp = Secp256k1::new();
        let mut rng = OsRng;

        let unblined_utxo = self
            .funding_utxo
            .unblind(&secp, self.swap_script.required_blinding_secret()?)?;
        let asset_id = unblined_utxo.asset;
        let out_abf = AssetBlindingFactor::new(&mut rng);
        let exp_asset = Asset::Explicit(asset_id);

        let (blinded_asset, asset_surjection_proof) =
            exp_asset.blind(&mut rng, &secp, out_abf, &[unblined_utxo])?;

        let output_value = Amount::from_sat(unblined_utxo.value)
            .checked_sub(Amount::from_sat(absolute_fees))
            .ok_or(Error::Protocol(format!(
                "Output value {} is less than fees {}",
                unblined_utxo.value, absolute_fees
            )))?;

        let final_vbf = ValueBlindingFactor::last(
            &secp,
            output_value.to_sat(),
            out_abf,
            &[(
                unblined_utxo.value,
                unblined_utxo.asset_bf,
                unblined_utxo.value_bf,
            )],
            &[(
                absolute_fees,
                AssetBlindingFactor::zero(),
                ValueBlindingFactor::zero(),
            )],
        );
        let explicit_value = elements::confidential::Value::Explicit(output_value.to_sat());
        let msg = elements::RangeProofMessage {
            asset: asset_id,
            bf: out_abf,
        };
        let ephemeral_sk = SecretKey::new(&mut rng);

        // assuming we always use a blinded address that has an extractable blinding pub
        let blinding_key = self
            .output_address
            .blinding_pubkey
            .ok_or(Error::Protocol("No blinding key in tx.".to_string()))?;
        let (blinded_value, nonce, rangeproof) = explicit_value.blind(
            &secp,
            final_vbf,
            blinding_key,
            ephemeral_sk,
            &self.output_address.script_pubkey(),
            &msg,
        )?;

        let tx_out_witness = TxOutWitness {
            surjection_proof: Some(Box::new(asset_surjection_proof)), // from asset blinding
            rangeproof: Some(Box::new(rangeproof)),                   // from value blinding
        };
        let payment_output: TxOut = TxOut {
            script_pubkey: self.output_address.script_pubkey(),
            value: blinded_value,
            asset: blinded_asset,
            nonce,
            witness: tx_out_witness,
        };
        let fee_output: TxOut = TxOut::new_fee(absolute_fees, asset_id);

        let refund_script = self.swap_script.refund_script();

        let lock_time = match refund_script
            .instructions()
            .filter_map(|i| {
                let ins = i.ok()?;
                if let Instruction::PushBytes(bytes) = ins {
                    if bytes.len() < 5_usize {
                        Some(LockTime::from_consensus(bytes_to_u32_little_endian(bytes)))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .next()
        {
            Some(r) => r,
            None => {
                return Err(Error::Protocol(
                    "Error getting timelock from refund script".to_string(),
                ))
            }
        };

        let mut refund_tx = Transaction {
            version: 2,
            lock_time,
            input: vec![refund_txin],
            output: vec![fee_output, payment_output],
        };

        if is_cooperative {
            refund_tx.input[0].witness = Self::stubbed_cooperative_witness();
        } else {
            refund_tx.input[0].sequence = Sequence::ZERO;

            let leaf_hash = TapLeafHash::from_script(&refund_script, LeafVersion::default());

            let sighash = SighashCache::new(&refund_tx).taproot_script_spend_signature_hash(
                0,
                &Prevouts::All(&[&self.funding_utxo]),
                leaf_hash,
                SchnorrSighashType::Default,
                self.genesis_hash,
            )?;

            let msg = Message::from_digest_slice(sighash.as_byte_array())?;

            let sig = secp.sign_schnorr(&msg, keys);

            let final_sig = SchnorrSig {
                sig,
                hash_ty: SchnorrSighashType::Default,
            };

            let control_block = match self
                .swap_script
                .taproot_spendinfo()?
                .control_block(&(refund_script.clone(), LeafVersion::default()))
            {
                Some(r) => r,
                None => return Err(Error::Taproot("Could not create control block".to_string())),
            };

            let mut script_witness = Witness::new();
            script_witness.push(final_sig.to_vec());
            script_witness.push(refund_script.as_bytes());
            script_witness.push(control_block.serialize());

            let witness = TxInWitness {
                amount_rangeproof: None,
                inflation_keys_rangeproof: None,
                script_witness: script_witness.to_vec(),
                pegin_witness: vec![],
            };

            refund_tx.input[0].witness = witness;
        }

        Ok(refund_tx)
    }

    fn stubbed_cooperative_witness() -> TxInWitness {
        let mut witness = Witness::new();
        // Stub because we don't want to create cooperative signatures here
        // but still be able to have an accurate size estimation
        witness.push([0; 64]);

        TxInWitness {
            amount_rangeproof: None,
            inflation_keys_rangeproof: None,
            script_witness: witness.to_vec(),
            pegin_witness: vec![],
        }
    }

    /// Calculate the size of a transaction.
    /// Use this before calling drain to help calculate the absolute fees.
    /// Multiply the size by the fee_rate to get the absolute fees.
    pub fn size(
        &self,
        keys: &Keypair,
        is_cooperative: bool,
        is_discount_ct: bool,
    ) -> Result<usize, Error> {
        let dummy_abs_fee = 1;
        let tx = match self.kind {
            SwapTxKind::Claim => {
                let preimage = Preimage::from_vec([0; 32].to_vec())?;
                self.create_claim(keys, &preimage, dummy_abs_fee, is_cooperative)?
            }
            SwapTxKind::Refund => self.create_refund(keys, dummy_abs_fee, is_cooperative)?,
        };
        Ok(tx_size(&tx, is_discount_ct))
    }

    /// Broadcast transaction to the network
    pub async fn broadcast<LC: LiquidClient + ?Sized>(
        &self,
        signed_tx: &Transaction,
        liquid_client: &LC,
    ) -> Result<String, Error> {
        liquid_client.broadcast_tx(signed_tx).await
    }
}

fn convert_schnorr_signature(
    schnorr_sig: secp256k1_musig::schnorr::Signature,
) -> bitcoin::secp256k1::schnorr::Signature {
    bitcoin::secp256k1::schnorr::Signature::from_slice(schnorr_sig.as_byte_array())
        .expect("signature size matches")
}

fn convert_pubkeys_for_musig(
    pubkeys: &[elements::secp256k1_zkp::PublicKey; 2],
) -> [secp256k1_musig::PublicKey; 2] {
    [
        convert_public_key(pubkeys[0]),
        convert_public_key(pubkeys[1]),
    ]
}

fn convert_xonly_key(key: secp256k1_musig::XOnlyPublicKey) -> bitcoin::XOnlyPublicKey {
    bitcoin::XOnlyPublicKey::from_slice(&key.serialize()[..]).expect("xonly key size matches")
}

fn convert_public_key(key: elements::secp256k1_zkp::PublicKey) -> secp256k1_musig::PublicKey {
    secp256k1_musig::PublicKey::from_slice(&key.serialize()[..]).expect("public key size matches")
}

impl SwapScriptCommon for LiquidSwapScript {
    fn swap_type(&self) -> SwapType {
        self.swap_type
    }

    /// Compute the Musig partial signature.
    /// This is used to cooperatively close a Submarine or Chain Swap.
    fn partial_sign(
        &self,
        keys: &Keypair,
        pub_nonce: &str,
        transaction_hash: &str,
    ) -> Result<(musig::PartialSignature, musig::PublicNonce), Error> {
        // Step 1: Start with a Musig KeyAgg Cache
        let pubkeys = [self.receiver_pubkey.inner, self.sender_pubkey.inner];
        let [a, b] = convert_pubkeys_for_musig(&pubkeys);

        let mut key_agg_cache = musig::KeyAggCache::new(&[&a, &b]);

        let tweak = Scalar::from_be_bytes(*self.taproot_spendinfo()?.tap_tweak().as_byte_array())?;

        let _ = key_agg_cache.pubkey_xonly_tweak_add(&tweak)?;

        let session_secret_rand = musig::SessionSecretRand::assume_unique_per_nonce_gen(rng_32b());

        let msg = hex_to_bytes32(transaction_hash)?;

        // Step 4: Start the Musig2 Signing session
        let mut extra_rand = [0u8; 32];
        OsRng.fill_bytes(&mut extra_rand);

        let (gen_sec_nonce, gen_pub_nonce) = key_agg_cache.nonce_gen(
            session_secret_rand,
            convert_public_key(keys.public_key()),
            &msg,
            Some(extra_rand),
        );

        let boltz_nonce = musig::PublicNonce::from_str(pub_nonce)?;

        let agg_nonce = musig::AggregatedNonce::new(&[&boltz_nonce, &gen_pub_nonce]);

        let musig_session = musig::Session::new(&key_agg_cache, agg_nonce, &msg);

        let partial_sig =
            musig_session.partial_sign(gen_sec_nonce, &convert_keypair(keys), &key_agg_cache);

        Ok((partial_sig, gen_pub_nonce))
    }
}

fn convert_keypair(keys: &Keypair) -> secp256k1_musig::Keypair {
    secp256k1_musig::Keypair::from_seckey_byte_array(keys.secret_bytes())
        .expect("keypair size matches")
}

fn tx_size(tx: &Transaction, is_discount_ct: bool) -> usize {
    match is_discount_ct {
        true => tx.discount_vsize(),
        false => tx.vsize(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_script(
        blinding_key: Option<ZKKeyPair>,
        asset_context: Option<LiquidAssetContext>,
        expected_amount: u64,
    ) -> LiquidSwapScript {
        let secp = Secp256k1::new();
        let receiver = ZKKeyPair::new(&secp, &mut OsRng);
        let sender = ZKKeyPair::new(&secp, &mut OsRng);
        LiquidSwapScript {
            swap_type: SwapType::Submarine,
            side: None,
            funding_addrs: None,
            hashlock: Preimage::random().hash160,
            receiver_pubkey: PublicKey::new(receiver.public_key()),
            locktime: LockTime::from_height(200).unwrap(),
            sender_pubkey: PublicKey::new(sender.public_key()),
            blinding_key,
            asset_context,
            expected_amount,
        }
    }

    fn explicit_output(script: Script, asset: elements::AssetId, value: u64) -> TxOut {
        TxOut {
            asset: Asset::Explicit(asset),
            value: Value::Explicit(value),
            nonce: elements::confidential::Nonce::Null,
            script_pubkey: script,
            witness: TxOutWitness::default(),
        }
    }

    #[macros::test_all]
    fn currency_asset_decoder_accepts_explicit_output_with_zero_blinders() {
        let asset = elements::AssetId::from_str(
            "1111111111111111111111111111111111111111111111111111111111111111",
        )
        .unwrap();
        let output = explicit_output(Script::new(), asset, 42);

        let secrets = decode_swap_output(&output, None, asset).unwrap();

        assert_eq!(secrets.asset, asset);
        assert_eq!(secrets.value, 42);
        assert_eq!(secrets.asset_bf, AssetBlindingFactor::zero());
        assert_eq!(secrets.value_bf, ValueBlindingFactor::zero());
    }

    #[macros::test_all]
    fn currency_asset_decoder_rejects_wrong_asset_key_and_mixed_encoding() {
        let secp = Secp256k1::new();
        let asset = LiquidChain::LiquidRegtest.bitcoin();
        let other = elements::AssetId::from_str(
            "1111111111111111111111111111111111111111111111111111111111111111",
        )
        .unwrap();
        let explicit = explicit_output(Script::new(), asset, 42);
        let key = ZKKeyPair::new(&secp, &mut OsRng);

        assert!(decode_swap_output(&explicit, None, other).is_err());
        assert!(decode_swap_output(&explicit, Some(key.secret_key()), asset).is_err());

        let mixed = TxOut {
            asset: Asset::new_confidential(&secp, asset, AssetBlindingFactor::zero()),
            ..explicit
        };
        assert!(decode_swap_output(&mixed, None, asset).is_err());
    }

    #[macros::test_all]
    fn explicit_and_confidential_address_validation_is_strict() {
        let secp = Secp256k1::new();
        let key = ZKKeyPair::new(&secp, &mut OsRng);
        let explicit = test_script(None, None, 42);
        let mut confidential = explicit.clone();
        confidential.blinding_key = Some(key);
        let explicit_address = explicit.to_address(LiquidChain::LiquidRegtest).unwrap();
        let confidential_address = confidential.to_address(LiquidChain::LiquidRegtest).unwrap();

        explicit
            .validate_address(LiquidChain::LiquidRegtest, explicit_address.to_string())
            .unwrap();
        confidential
            .validate_address(LiquidChain::LiquidRegtest, confidential_address.to_string())
            .unwrap();
        assert!(explicit
            .validate_address(LiquidChain::LiquidRegtest, confidential_address.to_string(),)
            .is_err());
        assert!(confidential
            .validate_address(LiquidChain::LiquidRegtest, explicit_address.to_string(),)
            .is_err());
    }

    #[macros::test_all]
    fn utxo_selection_skips_decoys_and_requires_script_asset_amount_and_txid() {
        let network = LiquidChain::LiquidRegtest;
        let swap_asset = elements::AssetId::from_str(
            "1111111111111111111111111111111111111111111111111111111111111111",
        )
        .unwrap();
        let script = test_script(
            None,
            Some(LiquidAssetContext {
                swap_asset,
                policy_asset: network.bitcoin(),
            }),
            42,
        );
        let swap_spk = script.to_address(network).unwrap().script_pubkey();
        let expected_txid = elements::Txid::from_str(
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        )
        .unwrap();
        let other_txid = elements::Txid::from_str(
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        )
        .unwrap();
        let candidates = vec![
            (
                OutPoint::new(expected_txid, 0),
                explicit_output(swap_spk.clone(), network.bitcoin(), 42),
            ),
            (
                OutPoint::new(expected_txid, 1),
                explicit_output(swap_spk.clone(), swap_asset, 41),
            ),
            (
                OutPoint::new(other_txid, 2),
                explicit_output(swap_spk.clone(), swap_asset, 42),
            ),
            (
                OutPoint::new(expected_txid, 3),
                explicit_output(swap_spk, swap_asset, 42),
            ),
        ];

        let selected = script
            .select_utxo(candidates, network, Some(expected_txid))
            .unwrap()
            .unwrap();

        assert_eq!(selected.0, OutPoint::new(expected_txid, 3));
    }

    fn prepared_lusdt_claim() -> (
        PreparedLiquidSpend,
        elements::AssetId,
        elements::AssetId,
        Keypair,
        Preimage,
    ) {
        let network = LiquidChain::LiquidRegtest;
        let policy_asset = network.bitcoin();
        let swap_asset = elements::AssetId::from_str(
            "1111111111111111111111111111111111111111111111111111111111111111",
        )
        .unwrap();
        let secp = bitcoin::secp256k1::Secp256k1::new();
        let keys = Keypair::new(&secp, &mut OsRng);
        let sender_keys = Keypair::new(&secp, &mut OsRng);
        let preimage = Preimage::random();
        let script = LiquidSwapScript {
            swap_type: SwapType::ReverseSubmarine,
            side: None,
            funding_addrs: None,
            hashlock: preimage.hash160,
            receiver_pubkey: PublicKey::new(keys.public_key()),
            locktime: LockTime::from_height(200).unwrap(),
            sender_pubkey: PublicKey::new(sender_keys.public_key()),
            blinding_key: None,
            asset_context: Some(LiquidAssetContext {
                swap_asset,
                policy_asset,
            }),
            expected_amount: 42,
        };
        let output_address = script.to_address(network).unwrap().to_unconfidential();
        let outpoint = OutPoint::new(elements::Txid::all_zeros(), 0);
        let funding_utxo = explicit_output(Script::new(), swap_asset, 42);
        let prepared = PreparedLiquidSpend::new(
            SwapTxKind::Claim,
            script,
            &output_address.to_string(),
            outpoint,
            funding_utxo,
            BlockHash::all_zeros(),
            3,
        )
        .unwrap();
        (prepared, swap_asset, policy_asset, keys, preimage)
    }

    fn fund_explicit_pset(
        prepared: &PreparedLiquidSpend,
        policy_asset: elements::AssetId,
        fee: u64,
    ) -> PartiallySignedTransaction {
        let mut pset = PartiallySignedTransaction::from_str(&prepared.template().pset).unwrap();
        let wallet_outpoint = OutPoint::new(
            elements::Txid::from_str(
                "2222222222222222222222222222222222222222222222222222222222222222",
            )
            .unwrap(),
            1,
        );
        let mut wallet_input = PsetInput::from_prevout(wallet_outpoint);
        wallet_input.sequence = Some(Sequence::ZERO);
        wallet_input.witness_utxo = Some(explicit_output(Script::new(), policy_asset, 10));
        wallet_input.asset = Some(policy_asset);
        wallet_input.amount = Some(10);
        wallet_input.final_script_witness = Some(vec![vec![1]]);
        // Insert before the HTLC input to exercise the real, non-zero swap index.
        pset.insert_input(wallet_input, 0);
        pset.add_output(PsetOutput::new_explicit(
            Script::from(vec![0x51]),
            10 - fee,
            policy_asset,
            None,
        ));
        pset.add_output(PsetOutput::new_explicit(
            Script::new(),
            fee,
            policy_asset,
            None,
        ));
        pset
    }

    fn explicit_payment_secrets(swap_asset: elements::AssetId) -> LiquidOutputSecrets {
        LiquidOutputSecrets {
            asset_id: swap_asset.to_string(),
            value: 42,
            asset_blinding_factor: AssetBlindingFactor::zero().to_string(),
            value_blinding_factor: ValueBlindingFactor::zero().to_string(),
        }
    }

    fn prepared_lusdt_refund() -> (
        PreparedLiquidSpend,
        elements::AssetId,
        elements::AssetId,
        Keypair,
    ) {
        let network = LiquidChain::LiquidRegtest;
        let policy_asset = network.bitcoin();
        let swap_asset = elements::AssetId::from_str(
            "1111111111111111111111111111111111111111111111111111111111111111",
        )
        .unwrap();
        let secp = bitcoin::secp256k1::Secp256k1::new();
        let receiver_keys = Keypair::new(&secp, &mut OsRng);
        let keys = Keypair::new(&secp, &mut OsRng);
        let script = LiquidSwapScript {
            swap_type: SwapType::Submarine,
            side: None,
            funding_addrs: None,
            hashlock: Preimage::random().hash160,
            receiver_pubkey: PublicKey::new(receiver_keys.public_key()),
            locktime: LockTime::from_height(200).unwrap(),
            sender_pubkey: PublicKey::new(keys.public_key()),
            blinding_key: None,
            asset_context: Some(LiquidAssetContext {
                swap_asset,
                policy_asset,
            }),
            expected_amount: 42,
        };
        let output_address = script.to_address(network).unwrap().to_unconfidential();
        let prepared = PreparedLiquidSpend::new(
            SwapTxKind::Refund,
            script,
            &output_address.to_string(),
            OutPoint::new(elements::Txid::all_zeros(), 0),
            explicit_output(Script::new(), swap_asset, 42),
            BlockHash::all_zeros(),
            3,
        )
        .unwrap();
        (prepared, swap_asset, policy_asset, keys)
    }

    #[macros::test_all]
    fn caller_funded_pset_signs_real_swap_index_and_preserves_wallet_witness() {
        let (prepared, swap_asset, policy_asset, keys, preimage) = prepared_lusdt_claim();
        assert_eq!(prepared.template().max_fee, 3);
        let pset = fund_explicit_pset(&prepared, policy_asset, 2);
        let funded = FundedLiquidPset {
            pset: pset.to_string(),
            payment_output_secrets: explicit_payment_secrets(swap_asset),
        };
        let tx = prepared.finalize_claim(funded, &keys, &preimage).unwrap();

        assert_eq!(tx.input.len(), 2);
        assert_eq!(tx.input[0].witness.script_witness, vec![vec![1]]);
        assert_eq!(tx.input[1].previous_output, prepared.funding_outpoint);
        assert_eq!(tx.input[1].witness.script_witness.len(), 4);
        assert_eq!(tx.output[0].value, Value::Explicit(42));
    }

    #[macros::test_all]
    fn caller_funded_refund_pins_timeout_and_adds_only_refund_witness() {
        let (prepared, swap_asset, policy_asset, keys) = prepared_lusdt_refund();
        let pset = fund_explicit_pset(&prepared, policy_asset, 2);
        let tx = prepared
            .finalize_refund(
                FundedLiquidPset {
                    pset: pset.to_string(),
                    payment_output_secrets: explicit_payment_secrets(swap_asset),
                },
                &keys,
            )
            .unwrap();
        assert_eq!(tx.lock_time, LockTime::from_height(200).unwrap());
        assert_eq!(tx.input[0].witness.script_witness, vec![vec![1]]);
        assert_eq!(tx.input[1].witness.script_witness.len(), 3);

        let mut changed = fund_explicit_pset(&prepared, policy_asset, 2);
        changed.global.tx_data.fallback_locktime = Some(LockTime::ZERO);
        assert!(prepared
            .finalize_refund(
                FundedLiquidPset {
                    pset: changed.to_string(),
                    payment_output_secrets: explicit_payment_secrets(swap_asset),
                },
                &keys,
            )
            .unwrap_err()
            .message()
            .contains("locktime"));
    }

    #[macros::test_all]
    fn caller_funded_pset_requires_policy_fee_input_with_typed_error() {
        let (prepared, swap_asset, _, keys, preimage) = prepared_lusdt_claim();
        let mut pset = PartiallySignedTransaction::from_str(&prepared.template().pset).unwrap();
        pset.add_output(PsetOutput::new_explicit(
            Script::new(),
            1,
            prepared.asset_context.policy_asset,
            None,
        ));
        let error = prepared
            .finalize_claim(
                FundedLiquidPset {
                    pset: pset.to_string(),
                    payment_output_secrets: explicit_payment_secrets(swap_asset),
                },
                &keys,
                &preimage,
            )
            .unwrap_err();
        assert!(matches!(error, Error::LiquidFeeAssetRequired));
        assert_eq!(error.name(), "liquid_fee_asset_required");
    }

    #[macros::test_all]
    fn caller_funded_pset_rejects_fee_cap_lusdt_skim_and_duplicate_input() {
        let (prepared, swap_asset, policy_asset, keys, preimage) = prepared_lusdt_claim();

        let over_cap = fund_explicit_pset(&prepared, policy_asset, 4);
        assert!(prepared
            .finalize_claim(
                FundedLiquidPset {
                    pset: over_cap.to_string(),
                    payment_output_secrets: explicit_payment_secrets(swap_asset),
                },
                &keys,
                &preimage,
            )
            .unwrap_err()
            .message()
            .contains("exceeds pinned cap"));

        let mut skim = fund_explicit_pset(&prepared, policy_asset, 2);
        skim.outputs_mut()[0].amount = Some(41);
        assert!(prepared
            .finalize_claim(
                FundedLiquidPset {
                    pset: skim.to_string(),
                    payment_output_secrets: explicit_payment_secrets(swap_asset),
                },
                &keys,
                &preimage,
            )
            .is_err());

        let mut duplicate = fund_explicit_pset(&prepared, policy_asset, 2);
        let cloned = duplicate.inputs()[0].clone();
        duplicate.add_input(cloned);
        assert!(prepared
            .finalize_claim(
                FundedLiquidPset {
                    pset: duplicate.to_string(),
                    payment_output_secrets: explicit_payment_secrets(swap_asset),
                },
                &keys,
                &preimage,
            )
            .unwrap_err()
            .message()
            .contains("duplicate inputs"));
    }

    #[macros::test_all]
    fn confidential_wallet_metadata_proofs_bind_asset_and_amount() {
        use elements::secp256k1_zkp::{RangeProof, SurjectionProof};

        let secp = Secp256k1::new();
        let policy_asset = LiquidChain::LiquidRegtest.bitcoin();
        let value = 10;
        let abf = AssetBlindingFactor::new(&mut OsRng);
        let vbf = ValueBlindingFactor::new(&mut OsRng);
        let Asset::Confidential(asset_commit) = Asset::new_confidential(&secp, policy_asset, abf)
        else {
            unreachable!()
        };
        let Value::Confidential(value_commit) =
            Value::new_confidential_from_assetid(&secp, value, policy_asset, vbf, abf)
        else {
            unreachable!()
        };
        let mut input = PsetInput::from_prevout(OutPoint::default());
        input.asset = Some(policy_asset);
        input.amount = Some(value);
        input.blind_asset_proof = Some(Box::new(
            SurjectionProof::blind_asset_proof(&mut OsRng, &secp, policy_asset, abf).unwrap(),
        ));
        input.blind_value_proof = Some(Box::new(
            RangeProof::blind_value_proof(
                &mut OsRng,
                &secp,
                value,
                value_commit,
                asset_commit,
                vbf,
            )
            .unwrap(),
        ));
        let witness_utxo = TxOut {
            asset: Asset::Confidential(asset_commit),
            value: Value::Confidential(value_commit),
            nonce: elements::confidential::Nonce::Null,
            script_pubkey: Script::new(),
            witness: TxOutWitness::default(),
        };

        assert_eq!(
            verify_input_metadata(&secp, &input, &witness_utxo, 1).unwrap(),
            (policy_asset, value)
        );
        input.amount = Some(value - 1);
        assert!(verify_input_metadata(&secp, &input, &witness_utxo, 1).is_err());
    }

    #[macros::test_all]
    fn caller_funded_pset_accepts_proven_confidential_input_payout_and_change() {
        use elements::secp256k1_zkp::{RangeProof, SurjectionProof};

        let (prepared, swap_asset, policy_asset, keys, preimage) = prepared_lusdt_claim();
        let secp = Secp256k1::new();
        let mut pset = PartiallySignedTransaction::from_str(&prepared.template().pset).unwrap();

        let wallet_abf = AssetBlindingFactor::new(&mut OsRng);
        let wallet_vbf = ValueBlindingFactor::new(&mut OsRng);
        let Asset::Confidential(wallet_asset_commit) =
            Asset::new_confidential(&secp, policy_asset, wallet_abf)
        else {
            unreachable!()
        };
        let Value::Confidential(wallet_value_commit) =
            Value::new_confidential_from_assetid(&secp, 10, policy_asset, wallet_vbf, wallet_abf)
        else {
            unreachable!()
        };
        let mut wallet_input = PsetInput::from_prevout(OutPoint::new(
            elements::Txid::from_str(
                "2222222222222222222222222222222222222222222222222222222222222222",
            )
            .unwrap(),
            1,
        ));
        wallet_input.sequence = Some(Sequence::ZERO);
        wallet_input.witness_utxo = Some(TxOut {
            asset: Asset::Confidential(wallet_asset_commit),
            value: Value::Confidential(wallet_value_commit),
            nonce: elements::confidential::Nonce::Null,
            script_pubkey: Script::from(vec![0x51]),
            witness: TxOutWitness::default(),
        });
        wallet_input.asset = Some(policy_asset);
        wallet_input.amount = Some(10);
        wallet_input.blind_asset_proof = Some(Box::new(
            SurjectionProof::blind_asset_proof(&mut OsRng, &secp, policy_asset, wallet_abf)
                .unwrap(),
        ));
        wallet_input.blind_value_proof = Some(Box::new(
            RangeProof::blind_value_proof(
                &mut OsRng,
                &secp,
                10,
                wallet_value_commit,
                wallet_asset_commit,
                wallet_vbf,
            )
            .unwrap(),
        ));
        wallet_input.final_script_witness = Some(vec![vec![1]]);
        pset.insert_input(wallet_input, 0);

        let payment_blinder = ZKKeyPair::new(&secp, &mut OsRng);
        pset.outputs_mut()[0].blinding_key = Some(PublicKey::new(payment_blinder.public_key()));
        pset.outputs_mut()[0].blinder_index = Some(0);
        let change_blinder = ZKKeyPair::new(&secp, &mut OsRng);
        let mut change = PsetOutput::new_explicit(
            Script::from(vec![0x52]),
            8,
            policy_asset,
            Some(PublicKey::new(change_blinder.public_key())),
        );
        change.blinder_index = Some(0);
        pset.add_output(change);
        pset.add_output(PsetOutput::new_explicit(
            Script::new(),
            2,
            policy_asset,
            None,
        ));

        let mut input_secrets = std::collections::HashMap::new();
        input_secrets.insert(
            0,
            TxOutSecrets {
                asset: policy_asset,
                asset_bf: wallet_abf,
                value: 10,
                value_bf: wallet_vbf,
            },
        );
        input_secrets.insert(
            1,
            TxOutSecrets {
                asset: swap_asset,
                asset_bf: AssetBlindingFactor::zero(),
                value: 42,
                value_bf: ValueBlindingFactor::zero(),
            },
        );
        let output_secrets = pset.blind_last(&mut OsRng, &secp, &input_secrets).unwrap();
        let (payment_abf, payment_vbf, _) = output_secrets.values().next().unwrap();
        let tx = prepared
            .finalize_claim(
                FundedLiquidPset {
                    pset: pset.to_string(),
                    payment_output_secrets: LiquidOutputSecrets {
                        asset_id: swap_asset.to_string(),
                        value: 42,
                        asset_blinding_factor: payment_abf.to_string(),
                        value_blinding_factor: payment_vbf.to_string(),
                    },
                },
                &keys,
                &preimage,
            )
            .unwrap();
        assert!(tx.output[0].asset.is_confidential());
        assert!(tx.output[0].value.is_confidential());
        assert!(tx.output[1].asset.is_confidential());
        assert_eq!(tx.output[2].value, Value::Explicit(2));
    }

    #[macros::test_all]
    fn test_tx_size() {
        // From https://github.com/ElementsProject/ELIPs/blob/main/elip-0200.mediawiki#test-vectors
        let tx: Transaction = elements::encode::deserialize(&hex::decode("0200000001017b85545c658d507ff56f315c77f910dd19cc9ceb7d5e1e4d3a3f8be4a91fe7440000000000fdffffff020bb6478c61c8f5f024ded219c967314685257f0ded894eaf626a00843a6ab80412091ee78237e38fb36c8be564ecd76e65f743065522f38f838367680ed7287b459103aabd97d4c8f3eac9555edfd2a709370b802335da478b6578501f72a4d100482716001455f4f701eec6059f956a40335e317a96a5e87ab5016d521c38ec1ea15734ae22b7c46064412829c0d0579f0a713d1c04ede979026f01000000000000000e00000000000000000347304402205d62bc013832eb6a631fe0285c49b7e27846e03189a245bec8f86346382282a702206c6e839b4b1d79d74662e432b724671402a6cfa2287911677c7061a3a32abe34012042c6504afda18a302bbf935f1dc646f71872a9a2fb5ed9e0cffb64588fd0d0a865a9141243397ee5e188bdcd17c9529c1382c7f8bc0fe987632102a3cd0d865794542994737e776dc3827a046c02ea2693f1d1f64315b3557bbb8b670395f72bb17521034a2e0343a515cf7d4a583d05bec3ee9fc16758cae791c10064fa92d65672d1fe68ac004301000177ce2a14a4f9e556fc846219827e1bc584caf9ef35e761dbf1f961a89b8285bde8fbe242c6984dd28719a792cd2e63535287db9a3b1fc4e4c5ae28cc5e8973d0fd4e10603300000000000000014cf45a01f0036bec883cdd4d5d8de1d7b3f2ec125733ce2e123ef3ff0085c50fd1b8cd3101c24fd8fff0bab803cda813aad9645ca6714ce768da75da09b58851585551c425e729d6faf4186a6659ea107f4ef35cc458dae565f1337af46cde218563eb3a756dc5d532717cc775fc0d04fbf4492070eb3cd9943a12fd07939d69a71090871e1ddf8fe716e2bc3f3364783cdb1d6a704325ca6c4334171563ae7bfcc9766ab848a65f47973753b2758b4404f17e54527080cfb980d1227f70cc0e77212d06aea909c7f2ac38f4a75c387464f8b70e33061f017a6fbbccf0673d08aebae2a1ce6cf9dd8c98791b1f4d653788b2ed6dd65cf9795eac568744e386d68c89d973ca079298f8d292b6bee71fad94a0f83aaf070ccfeb6c6de20baf8c6f1083dcdd539fae6ed74832100ea7c07296c0af2201523c3abf8b784ca8a235556d5bae668f17d9a353fd49dbae623ca44830a8fc4963419e49a9dc99bf87ea0414be3b43a6eab8ce54695d66887b261c08252a501d0c78d30be1ae3fc10f557f4d228ef38da496b22c5fa79d92e2c190b9d31f286dc0e3c8489fcb8e0603f8b93a6eb1ec726a7e0015e70407da186d85b290b054747276a8928443e1108cb67738d156787d20553c39fa0449f95addbf42170fdab8107d1f93fcd841964b6e6c4c140d0c4ed1463835e603f5012a4aafd5b038ceb9b4a5b7e2688cfd8c4f2bfafaf0bb5bb1aa7a7f13bd47ff3da57c4c88b741fd9ff97abc23d4047f690d59c4c67494f47125fe0f626ad409a92d72907ad0b1762b5271f474fa552d9139fcb1103db24f7a29726a5e41a6dbc43590c14a62eb1b2aa0f160134c42c6c87c696e7c42546bb72f9f531729555d01c529570553aeec70709c3a4f9aacf810d5018f776af48b93eff8e120242105c06a32e64bfc825fde488c99d5845adba2cf349717f64e488852cca73cc5813b7872f7e89d24b4bfafdf75faa368375d5bfdd8b8a7ad641703cbff131616c77e79d8f78c5fe63810781db44fb1fa5cc9387cf0de6807d1a3d5e3d8f9ec7418bbb1d4e10b1fcdb300abd8625b4e24842f1f4c4e567fe9f8c6e9d314757d4568889bccc740fb36f0270804cc11c0044093ab9586ed034cd1eb70bacdedd573750794f0286dfb91c91308e507147ea8e8534c655b931f4e68543e93c57cf2f2159e021739943e40c0dbc8a68193218d40d71e0956b00b4a01fa9c06e67ea55e0213fab48a8dfcf3a047e8c438e7c94fc195026cec82ad532e2aa5970a9fe6c03d9088d0ab45e0b9c7bf9597bd2db93ef7d7f139c291f59e03cda1a5f9a793eb7ec6d50fa9482b712500b5e5a780319769836f7053e3c5a3276a7d65467578a7fbf9079fb5c6bb1b0558acbf3cd896644d42a7b0fd87b12b571b3d8122b1c254750bf9b097d0ec5ed31f9af7db9571f706f5909f0ef2fdcdb255a0795f5c28b70fd1d25b74eb2524ae8f47756875ff439a2b2769adc844312c4ac7bde16b561e62ee3069d25718bf6c2e11ffbb83c863a51c52ff4ead581dd6b1ff0913905163683b97ecbad003a1c71469050eed5ad79e9bb44179b90b8e6b0e6a61a0ed4e919cb96c2615b61cf93905adc3e6e2a127bd661f05e928a45bc1c0599c41450dea0182043b977fcfcf3620f765d3aab13cbe684028dc78a4bd02324427379735934ab4cb821623f49e3af05391c1b7acfe8be33c9201efeded50838ff216d6744d61e8d1d600260c8f7275a46764ac9392132f0b3661e5e92e9daa87b9329d9c89353f40a130bcf8611cce25335f9f1c1208ae1bdc47d96c3f83170a7d27367a043debdfd0e43776d330d1f7a806b32c4363d1dca14715dae4f4d1c99a92673954094e61387080353974097adfde15de4009caa28d42703fdb56fcdac47bd9c5e3bad2fbf90b4a3fab4d89a9933e445ba85f759cc149101f5045a6f3a6d741424318249d96277cea3dc0c4814763d727c72a1867618ac05e5ff103b985cc6f78829bae92794680a51c4b7f7f8b88e39ddd4471890914594f3f03ae668d501732ea77b3eb1fb38b5ad9efdac8775e0995c60a3949e84d2298ea3463aaa16d5ff633da654463e90004915ccc19663c87e006fcd05e904b85b71428d79913e3afdecb7ad51a66f7dcb738d028b62b307025d524320dbe064330da5cbd70467635cf492197c7be3513363b4000bf176827011b2894d33dc9d806b2526a6e91cc1cf0582c5330484b8d48be4855c1859a5b20cab6d08d95b42b57fc709dcb637ba9c6e70b72c473af88ebe8723fe94a0d5ee5d483f19c3b2aade19bafed774b786c0d24383fe0f71c085655f4bd78cb36da83b5429576576c0718b4549efe5b8f602c543c3a8e3d86f19b70d6be1fb39b7cbbac6fcf6d80d69c00ed44dbed1b8555593bd6dcf9ddd519f9325f6faa146d4b631cc6ee418ef9d07a0036fb26a792e7733ec0b58d9f0ebba9ea9493fa026bab62f70381e534c8c3b349be651e9fd5d472b3cbf8f7e912b7030a1992df35e17f4c5aa54f1632464a7c3b0dd133da8d436205bf45d8ded924e35b366803ee52a3d1c85d9f4f976785270dafb63d2cd5052328ed2e5381e9a6e9d8409675c2a9a43c74b07e8a3df8043b2b6d42832cabfcd495b8b30727346990fbc79e436d7ba4d7035603ab98532c5497ef493511e498b1b9c5ff413e919ab6f3cd6acc472f6a39ad0a8c9677ac9a5380a6bebbaaf13a114d097efbf140acad7edecc758bb070fa0b88bb0646d3bed911414a3f10b12bf8372d66f4525f9a8a66d7bf2b5d364119a687e5f416511c27659cf70969863ed7f80e80a4f2e55bf25721e1ab415305b66bfc25b9630a265b553d3e806807f23ec1e2a5f657dbd73a4a36e95e6616faa6aefc5143ca29b0e4bc9eb1042d99c74115d96a2eec5e7fb8c3f598d4df8fa8953e96689651a705dd3f385cd27e0173baca570ce53001cdb002e4476e6af47b9a891f84f7c1c472cce3cd4a70a40c298819f6d75e6adac193798c740c9f5f57fee4df5d140cce8ee4152c17784899003dc000cd2e7c7f23e74da085b254e0843d97d147e44ab3ba12e308925fc6ab0460c7ceb107b0900cef5ff939bc3fe5640f0bb11597c561be275fc8b5b85f5e38a3c12ea26b5b7b32e407685db70d16a3ce51043d4009a647fd3656a54adcbd4d1baa6d89881973fe32faf071123de1712e85db628bdd987566b362845d0c5f818547ec2d1f7c668cae44f0bec74c6663134dd0273c3363f31901903e4e976a447af96f6f521059fb6b892a0599cf7aae457df3aed72f1f55e145332c91430a2f8184bb917d317f8d9c4b6769b9a3a0ac5baea88b39b8f7662ecc16585e7166f61a948f48e6d30c2cfd82820cccdf5e722db2156bd848ea4d13c92544d1d9064414a305215a8271631ffebf08cdf0bcbbbd939f78eafec0d7238bdb90f211d6c44589187d1a501eef7d0b6118e028afcf76ffda95a43e2211206d9d50d34c3e33a6c991952ccd73e722802a14227692f037bba585e73cb9a6cd7556f9ec2158f197a51e3884afb8e59eaa8e7ac3568d88b27b2a5ab8cd72648193ff6068e4d481c58c117e2adda564d5a49f6b992ff6f938acb283e7baf704c71861d60b263f6c6684d7544878b7aca942af8b3a70ae0def309b68fac2aed2b11ba753d7b47f7369805e5b3b9b41d22196e2cc098ece59bdf5231b03fba8adae08fee227a582490b0db34c115620c72afb6fcb507397d1333ea19e7969b729bc2733e6546d2d9f3edb08f9c74201f9ed4e3fcb446cc3fd688b1345e97b32492c9173fa71df2772bd825506ddd6447e9f9e8ece0ffb860e1c755bcf2400deef094219795d4ee84acc34dedc9a3b3adf7fc81733bc511b8edcb54769400940b53471d8e82cb82d9967a97297bdd87f165968ea046291234da176efd20889aa4c07179df83cb500b40bdb96b0c27f2bfa57353268b776740432d29f1761fee77755c7b219def785a42b683e1f70240ec45cdf660e894d4fb541d0511547c9a2c503cf605d72ea7f2abaee4e8adc222a82f4b86c34ad8b25e2932df02f0090d2dbf8817c44659b1245d5579277ad406c538914f90dbaefdd110c5ca0d63a24706cd51096ec19f819c446c9fcb55b777ae633f0257dc4d1b293e6ef68ea7867d852058212a0a9ace9442422a638f73dfb14cc4354b6481ee6591037e7287e962037d963b38a7e4ec12b30e0f6e0ee4d8c30d288e99e22e43b4c795c51d66cc4225c5cab3685b1b3a6fd3a82dfc355634b347cc4f4e55413728fb67fb9f34d3f7e4ecce3254ea843ab361b0f652faa9e54470e3e414c1bb2593e36d88109c36dfab505a16c19152fe021de608c6b3d924c981231ea9cf1cf8c93e53f0df78033e81fdb578a45b7dc4f3f0f68feedc78ec7c347f91a0464bccd58aa2fc11016e88cbaddfb22112edad752792af12fa550be3e6f15d69a6a9d547ab5381b93c58c12753b8085d9e17ed1f2519cc5cb756e3777ea9f8e49a6141460f8f6ced8d12d13d950691479e1207ed35ab71554122beb215a0fb6b34b90784f4be6bd6fbf93daf9d3bc4640bc52a662e750ce361c12c1bfa2ca4e2c784cbf70c406587b2ebd69faa7a891aca63d600247ad7dde426c1ef4e3b22a072ff8eb69c1b1cb30c605112786546c48cf1c4821b5bc0d0bd44ba83b05656b6e19a3d1a76931d983dd39efcc64298e892858e847e99519c1fa25b1998839788c5852b94202d803639d69058604374f76769670a60269dbc0688cea2d9d8672212b93ca501fbf6f7dfefad058e4bd0e0da1cff41b2f408c980f29a49b03efa9e3edef091d7df7529b6b5e8f7d43d103681cd7c38d02a431b15d539e9a3cf44dc71621664e756ad6404ba185b5e20c82760c488fde4253fb52ab850484a082e7ca275f475012be9c8d16d6b4a2c9d863440d5e113d18bbf42f128462764a99ca90af4fde890aee138fe4cbb45658eacd9d38c8a1fb4499c043cc25af87e6a650f38149ab018cc49f50bbd085e2a0ba3eeecde5764f7997748a660593191977792d7176e4c2ff0113d67b9abe8fbc10f364c6fa68e52a455aa56ff15099c6efb6b5812972380d5b8e256b0feb1190835b7d076744c1b5b738c710a07a32676a15d96583e89e39eb4ff08cf02c6e2ad540c2b66299afe01bf2e50c81465a04d229a07c58ffd25a6cd9288110045526b376548d373273e6227d117d491020fd68e366ed697a0d30a5bdff25fa9a5800aa534a3669215dfa8f30960f142a8ae7ffcb654ca60aa7dc8a586670f9db37d05644ff5f934785c5433e605f3fbd0340e168511e209a0aedd8b18f3b948eb58051136d155f53b0e2e027361330e005f83f3a72dcc5d9161dd4b1e6abd16635dc0887dcc833a1fb59c10e0b8bea2536e7acd58d5e11179d13a24dc4292624c527266351b9a48893b956ffe545c8d2c1563805addef2a82134c9c686449d83471f22c1e14601895e854a5f854230e4fb4ed4f9a7ee22e83234be6c5bb19d200c16543468f186ae11cba84ae1aeda5136f7f5b380d02ddb9cbe2c5f5bb39138fa29b2ceb549d2e337eba10171fc237473351cf8e5989c193ef0100c75778ad0c05b64b614067c9a70680c818a566c4ba5e2991eedfe165199a55b0bef1333988f2add167e268db389c2d25bd85eedff9e6851e3df84c9e41128b5a76869c086fcf9275b1d51af02e4a92b66850785319dbf004a29594e32d12ca42da69fac69f886f963409ce1d4514d1ab9e915e071887e7f316b15014d083769afea374e0771f74f632db5ed7d7352546ed686e3ee161cd263dafc2acab74a67a5721f923f9b07c647c2a04f7d1c2f831d4319a60b16ed4c995e35ccbc291ff647a382976ba5a957547b0000").unwrap()).unwrap();

        assert_eq!(tx_size(&tx, false), 1333);
        assert_eq!(tx_size(&tx, true), 216);
    }
}
