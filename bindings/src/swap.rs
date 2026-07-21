use crate::boltz::BoltzApiClientV2;
use crate::boltz::Error;
use crate::network::ChainClient;
use crate::util::Preimage;
use bitcoin::hex::DisplayHex;
use bitcoin::key::{rand, Keypair, PublicKey};
use bitcoin::secp256k1::SecretKey;
use kaleidoswap_sdk::boltz::ChainSwapDetails;
use kaleidoswap_sdk::boltz::{CreateReverseResponse, CreateSubmarineResponse, Side};
use kaleidoswap_sdk::fees::Fee;
use kaleidoswap_sdk::network::Chain;
use kaleidoswap_sdk::swaps::{self as swaps_bitcoin};
use std::str::FromStr;
use std::sync::Arc;

#[derive(Debug, uniffi::Object)]
pub struct SwapScript(swaps_bitcoin::SwapScript);

#[derive(uniffi::Record)]
pub struct SwapTransactionParams {
    pub output_address: String,
    pub fee: Fee,
    pub swap_id: String,
    pub keys: Arc<KeyPair>,
    pub chain_client: Arc<ChainClient>,
    pub boltz_api: Arc<BoltzApiClientV2>,
    #[uniffi(default = None)]
    pub options: Option<TransactionOptions>,
}

#[derive(uniffi::Record)]
pub struct LiquidPsetParams {
    pub output_address: String,
    pub max_fee: u64,
    pub quoted_fee_cap: u64,
    pub swap_id: String,
    pub chain_client: Arc<ChainClient>,
    pub boltz_api: Arc<BoltzApiClientV2>,
    /// Optional locally available Liquid lockup transaction. Supplying it
    /// avoids depending on API/indexer transaction discovery.
    #[uniffi(default = None)]
    pub lockup_tx: Option<Arc<BtcLikeTransaction>>,
}

#[derive(Clone, uniffi::Record)]
pub struct LiquidPsetTemplate {
    pub pset: String,
    pub swap_input_index: u32,
    pub payment_output_index: u32,
    pub swap_asset_id: String,
    pub policy_asset_id: String,
    pub amount: u64,
    pub max_fee: u64,
}

impl From<swaps_bitcoin::liquid::LiquidPsetTemplate> for LiquidPsetTemplate {
    fn from(template: swaps_bitcoin::liquid::LiquidPsetTemplate) -> Self {
        Self {
            pset: template.pset,
            swap_input_index: template.swap_input_index,
            payment_output_index: template.payment_output_index,
            swap_asset_id: template.swap_asset_id,
            policy_asset_id: template.policy_asset_id,
            amount: template.amount,
            max_fee: template.max_fee,
        }
    }
}

#[derive(Clone, uniffi::Record)]
pub struct LiquidOutputSecrets {
    pub asset_id: String,
    pub value: u64,
    pub asset_blinding_factor: String,
    pub value_blinding_factor: String,
}

impl From<LiquidOutputSecrets> for swaps_bitcoin::liquid::LiquidOutputSecrets {
    fn from(secrets: LiquidOutputSecrets) -> Self {
        Self {
            asset_id: secrets.asset_id,
            value: secrets.value,
            asset_blinding_factor: secrets.asset_blinding_factor,
            value_blinding_factor: secrets.value_blinding_factor,
        }
    }
}

#[derive(Clone, uniffi::Record)]
pub struct FundedLiquidPset {
    pub pset: String,
    pub payment_output_secrets: LiquidOutputSecrets,
}

impl From<FundedLiquidPset> for swaps_bitcoin::liquid::FundedLiquidPset {
    fn from(funded: FundedLiquidPset) -> Self {
        Self {
            pset: funded.pset,
            payment_output_secrets: funded.payment_output_secrets.into(),
        }
    }
}

#[derive(Debug, uniffi::Object)]
pub struct PreparedLiquidSpend(swaps_bitcoin::liquid::PreparedLiquidSpend);

#[uniffi::export]
impl PreparedLiquidSpend {
    #[uniffi::method]
    pub fn template(&self) -> LiquidPsetTemplate {
        self.0.template().into()
    }

    #[uniffi::method]
    pub fn finalize_claim(
        &self,
        funded_pset: FundedLiquidPset,
        keys: &KeyPair,
        preimage: &Preimage,
    ) -> Result<BtcLikeTransaction, Error> {
        self.0
            .finalize_claim(funded_pset.into(), &keys.inner, &preimage.0)
            .map(swaps_bitcoin::BtcLikeTransaction::liquid)
            .map(BtcLikeTransaction)
            .map_err(Into::into)
    }

    #[uniffi::method]
    pub fn finalize_refund(
        &self,
        funded_pset: FundedLiquidPset,
        keys: &KeyPair,
    ) -> Result<BtcLikeTransaction, Error> {
        self.0
            .finalize_refund(funded_pset.into(), &keys.inner)
            .map(swaps_bitcoin::BtcLikeTransaction::liquid)
            .map(BtcLikeTransaction)
            .map_err(Into::into)
    }
}

impl<'a> From<&'a SwapTransactionParams> for swaps_bitcoin::SwapTransactionParams<'a> {
    fn from(params: &'a SwapTransactionParams) -> Self {
        swaps_bitcoin::SwapTransactionParams {
            keys: params.keys.inner,
            output_address: params.output_address.clone(),
            fee: params.fee,
            swap_id: params.swap_id.clone(),
            options: params.options.clone().map(|o| {
                let mut options =
                    swaps_bitcoin::TransactionOptions::default().with_cooperative(o.cooperative);
                if let Some(chain_claim) = o.chain_claim {
                    options = options.with_chain_claim(
                        chain_claim.keys.inner,
                        chain_claim.lockup_script.0.clone(),
                    );
                }
                options
            }),
            chain_client: &params.chain_client.0,
            boltz_api: &params.boltz_api.inner,
        }
    }
}

#[uniffi::export(async_runtime = "tokio")]
impl SwapScript {
    #[uniffi::constructor]
    pub fn from_submarine(
        chain: Chain,
        create_swap_response: &CreateSubmarineResponse,
        our_pubkey: PublicKey,
    ) -> Result<Self, Error> {
        let script = swaps_bitcoin::SwapScript::submarine_from_swap_resp(
            chain,
            create_swap_response,
            our_pubkey,
        )?;
        Ok(Self(script))
    }

    #[uniffi::constructor]
    pub fn from_reverse(
        chain: Chain,
        reverse_response: &CreateReverseResponse,
        our_pubkey: PublicKey,
    ) -> Result<Self, Error> {
        let script =
            swaps_bitcoin::SwapScript::reverse_from_swap_resp(chain, reverse_response, our_pubkey)?;
        Ok(Self(script))
    }

    #[uniffi::constructor]
    pub fn from_chain(
        chain: Chain,
        side: Side,
        chain_swap_details: ChainSwapDetails,
        our_pubkey: PublicKey,
    ) -> Result<Self, Error> {
        let script = swaps_bitcoin::SwapScript::chain_from_swap_resp(
            chain,
            side,
            chain_swap_details,
            our_pubkey,
        )?;
        Ok(Self(script))
    }

    #[uniffi::method]
    pub async fn construct_claim(
        &self,
        preimage: &Preimage,
        params: &SwapTransactionParams,
    ) -> Result<BtcLikeTransaction, Error> {
        let tx = self.0.construct_claim(&preimage.0, params.into()).await?;
        Ok(BtcLikeTransaction(tx))
    }

    #[uniffi::method]
    pub async fn construct_refund(
        &self,
        params: &SwapTransactionParams,
    ) -> Result<BtcLikeTransaction, Error> {
        let tx = self.0.construct_refund(params.into()).await?;
        Ok(BtcLikeTransaction(tx))
    }

    #[uniffi::method]
    pub async fn prepare_liquid_claim(
        &self,
        params: &LiquidPsetParams,
    ) -> Result<PreparedLiquidSpend, Error> {
        let prepared = self
            .0
            .prepare_liquid_claim(swaps_bitcoin::LiquidPsetParams {
                output_address: params.output_address.clone(),
                max_fee: params.max_fee,
                quoted_fee_cap: params.quoted_fee_cap,
                swap_id: params.swap_id.clone(),
                chain_client: &params.chain_client.0,
                boltz_api: &params.boltz_api.inner,
                options: params.lockup_tx.as_ref().map(|tx| {
                    swaps_bitcoin::TransactionOptions::default().with_lockup_tx(tx.0.clone())
                }),
            })
            .await?;
        Ok(PreparedLiquidSpend(prepared))
    }

    #[uniffi::method]
    pub async fn prepare_liquid_refund(
        &self,
        params: &LiquidPsetParams,
    ) -> Result<PreparedLiquidSpend, Error> {
        let prepared = self
            .0
            .prepare_liquid_refund(swaps_bitcoin::LiquidPsetParams {
                output_address: params.output_address.clone(),
                max_fee: params.max_fee,
                quoted_fee_cap: params.quoted_fee_cap,
                swap_id: params.swap_id.clone(),
                chain_client: &params.chain_client.0,
                boltz_api: &params.boltz_api.inner,
                options: params.lockup_tx.as_ref().map(|tx| {
                    swaps_bitcoin::TransactionOptions::default().with_lockup_tx(tx.0.clone())
                }),
            })
            .await?;
        Ok(PreparedLiquidSpend(prepared))
    }

    #[uniffi::method]
    pub async fn submarine_cooperative_claim(
        &self,
        swap_id: &String,
        keys: &KeyPair,
        invoice: &str,
        boltz_api: &BoltzApiClientV2,
    ) -> Result<(), Error> {
        self.0
            .submarine_cooperative_claim(swap_id, &keys.inner, invoice, &boltz_api.inner)
            .await?;
        Ok(())
    }
}
#[derive(uniffi::Record, Clone)]
pub struct ChainClaim {
    pub keys: Arc<KeyPair>,
    pub lockup_script: Arc<SwapScript>,
}

#[derive(uniffi::Record, Clone)]
pub struct TransactionOptions {
    #[uniffi(default = true)]
    pub cooperative: bool,
    #[uniffi(default = None)]
    pub chain_claim: Option<ChainClaim>,
}

#[uniffi::remote(Enum)]
pub enum Fee {
    // In sat/vByte
    Relative(f64),
    // In satoshis
    Absolute(u64),
}

#[derive(uniffi::Object)]
pub struct BtcLikeTransaction(pub(crate) swaps_bitcoin::BtcLikeTransaction);

#[uniffi::export]
impl BtcLikeTransaction {
    #[uniffi::method]
    pub fn hex(&self) -> String {
        match &self.0 {
            swaps_bitcoin::BtcLikeTransaction::Bitcoin(tx) => {
                bitcoin::consensus::serialize(tx).to_lower_hex_string()
            }
            swaps_bitcoin::BtcLikeTransaction::Liquid(tx) => {
                elements::encode::serialize(tx).to_lower_hex_string()
            }
        }
    }

    #[uniffi::method]
    pub fn txid(&self) -> String {
        match &self.0 {
            swaps_bitcoin::BtcLikeTransaction::Bitcoin(tx) => tx.compute_txid().to_string(),
            swaps_bitcoin::BtcLikeTransaction::Liquid(tx) => tx.txid().to_string(),
        }
    }
}

#[derive(uniffi::Object)]
pub struct KeyPair {
    inner: Keypair,
}

#[uniffi::export]
impl KeyPair {
    #[uniffi::constructor]
    pub fn new() -> Self {
        let secp = bitcoin::secp256k1::Secp256k1::new();
        let key = Keypair::new(&secp, &mut rand::thread_rng());
        KeyPair { inner: key }
    }

    #[uniffi::constructor]
    pub fn from_secret_key(secret: SecretKey) -> Self {
        let secp = bitcoin::secp256k1::Secp256k1::new();
        KeyPair {
            inner: Keypair::from_secret_key(&secp, &secret),
        }
    }

    #[uniffi::method]
    pub fn secret(&self) -> SecretKey {
        self.inner.secret_key()
    }

    #[uniffi::method]
    pub fn public(&self) -> PublicKey {
        self.inner.public_key().into()
    }
}

impl Default for KeyPair {
    fn default() -> Self {
        Self::new()
    }
}

uniffi::custom_type!(PublicKey, String, {
    remote,
    try_lift: |val| match PublicKey::from_str(val.as_str()) {
        Ok(key) => Ok(key),
        Err(e) => Err(e.into())
    },
    lower: |val| val.to_string(),
});

uniffi::custom_type!(SecretKey, String, {
    remote,
    try_lift: |val| match SecretKey::from_str(val.as_str()) {
        Ok(key) => Ok(key),
        Err(e) => Err(e.into())
    },
    lower: |val| val.secret_bytes().to_upper_hex_string(),
});
