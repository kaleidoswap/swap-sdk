use core::fmt;
use std::str::FromStr;

use crate::error::Error;
use elements::{AddressParams, AssetId};

#[cfg(feature = "electrum")]
#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
pub mod electrum;

#[cfg(feature = "esplora")]
pub mod esplora;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Chain {
    Bitcoin(BitcoinChain),
    Liquid(LiquidChain),
}

/// Asset selected for a swap independently from the chain network.
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Currency {
    Btc,
    LBtc,
    LUsdt,
}

impl fmt::Display for Currency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Currency::Btc => write!(f, "BTC"),
            Currency::LBtc => write!(f, "L-BTC"),
            Currency::LUsdt => write!(f, "L-USDT"),
        }
    }
}

impl fmt::Display for Chain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Chain::Bitcoin(_) => write!(f, "BTC"),
            Chain::Liquid(_) => write!(f, "L-BTC"),
        }
    }
}

impl Chain {
    /// Resolve an optional asset selection for this chain.
    ///
    /// Omitting the currency preserves the legacy mapping: Bitcoin uses BTC
    /// and Liquid uses L-BTC.
    pub fn resolve_currency(self, currency: Option<Currency>) -> Result<Currency, Error> {
        let currency = currency.unwrap_or(match self {
            Chain::Bitcoin(_) => Currency::Btc,
            Chain::Liquid(_) => Currency::LBtc,
        });

        match (self, currency) {
            (Chain::Bitcoin(_), Currency::Btc)
            | (Chain::Liquid(_), Currency::LBtc | Currency::LUsdt) => Ok(currency),
            (chain, currency) => Err(Error::Protocol(format!(
                "Currency {currency} is not valid for chain {chain}"
            ))),
        }
    }
}

impl From<BitcoinChain> for Chain {
    fn from(value: BitcoinChain) -> Self {
        Chain::Bitcoin(value)
    }
}

impl From<LiquidChain> for Chain {
    fn from(value: LiquidChain) -> Self {
        Chain::Liquid(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BitcoinChain {
    Bitcoin,
    BitcoinTestnet,
    /// Signet — distinct from [`BitcoinChain::BitcoinTestnet`] even though the
    /// two share an address encoding.
    ///
    /// A signet is defined by its challenge, so "signet" is a family rather
    /// than one chain. This SDK's defaults target
    /// [Mutinynet](https://mutinynet.com), the signet the KaleidoSwap maker is
    /// deployed on; a vanilla-signet explorer serves a different chain and
    /// cannot see these transactions.
    BitcoinSignet,
    BitcoinRegtest,
}

impl From<BitcoinChain> for bitcoin::Network {
    fn from(value: BitcoinChain) -> Self {
        match value {
            BitcoinChain::Bitcoin => Self::Bitcoin,
            BitcoinChain::BitcoinTestnet => Self::Testnet,
            BitcoinChain::BitcoinSignet => Self::Signet,
            BitcoinChain::BitcoinRegtest => Self::Regtest,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiquidChain {
    Liquid,
    LiquidTestnet,
    LiquidRegtest,
}

const ASSET_ID_REGTEST: &str = "5ac9f65c0efcc4775e0baec4ec03abdde22473cd3cf33c0419ca290e0751b225";
const ASSET_ID_TESTNET: &str = "144c654344aa716d6f3abcc1ca90e5641e4e2a7f633bc09fe3baf64585819a49";

impl LiquidChain {
    pub fn bitcoin(self) -> AssetId {
        match self {
            LiquidChain::Liquid => AssetId::LIQUID_BTC,
            LiquidChain::LiquidTestnet => AssetId::from_str(ASSET_ID_TESTNET).unwrap(),
            LiquidChain::LiquidRegtest => AssetId::from_str(ASSET_ID_REGTEST).unwrap(),
        }
    }
}

impl From<LiquidChain> for &'static AddressParams {
    fn from(value: LiquidChain) -> Self {
        match value {
            LiquidChain::Liquid => &AddressParams::LIQUID,
            LiquidChain::LiquidTestnet => &AddressParams::LIQUID_TESTNET,
            LiquidChain::LiquidRegtest => &AddressParams::ELEMENTS,
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Network {
    Mainnet,
    Testnet,
    /// Signet — the network the KaleidoSwap maker is deployed on. Chain
    /// defaults resolve to Mutinynet; see [`BitcoinChain::BitcoinSignet`].
    Signet,
    Regtest,
}

impl From<Network> for BitcoinChain {
    fn from(value: Network) -> Self {
        match value {
            Network::Mainnet => BitcoinChain::Bitcoin,
            Network::Testnet => BitcoinChain::BitcoinTestnet,
            Network::Signet => BitcoinChain::BitcoinSignet,
            Network::Regtest => BitcoinChain::BitcoinRegtest,
        }
    }
}

impl From<Network> for LiquidChain {
    fn from(value: Network) -> Self {
        match value {
            Network::Mainnet => LiquidChain::Liquid,
            Network::Testnet => LiquidChain::LiquidTestnet,
            // Liquid has no signet, so signet BTC pairs with Liquid testnet —
            // the same asymmetry Boltz testnet has. The KaleidoSwap maker
            // confirms it: its L-BTC side tracks the Liquid testnet tip.
            Network::Signet => LiquidChain::LiquidTestnet,
            Network::Regtest => LiquidChain::LiquidRegtest,
        }
    }
}

impl From<Network> for bitcoin::Network {
    fn from(value: Network) -> Self {
        match value {
            Network::Mainnet => Self::Bitcoin,
            Network::Testnet => Self::Testnet,
            Network::Signet => Self::Signet,
            Network::Regtest => Self::Regtest,
        }
    }
}

impl From<Chain> for Network {
    fn from(value: Chain) -> Self {
        match value {
            Chain::Bitcoin(_) => Network::Mainnet,
            Chain::Liquid(_) => Network::Mainnet,
        }
    }
}

#[macros::async_trait]
pub trait BitcoinClient: Send + Sync {
    async fn get_address_balance(&self, address: &bitcoin::Address) -> Result<(u64, i64), Error>;

    async fn get_address_utxos(
        &self,
        address: &bitcoin::Address,
    ) -> Result<Vec<(bitcoin::OutPoint, bitcoin::TxOut)>, Error>;

    async fn get_tx(&self, txid: bitcoin::Txid) -> Result<bitcoin::Transaction, Error>;

    async fn broadcast_tx(&self, signed_tx: &bitcoin::Transaction) -> Result<bitcoin::Txid, Error>;

    fn network(&self) -> BitcoinChain;
}

#[macros::async_trait]
pub trait LiquidClient: Send + Sync {
    async fn get_address_utxos(
        &self,
        address: &elements::Address,
    ) -> Result<Vec<(elements::OutPoint, elements::TxOut)>, Error>;

    async fn get_genesis_hash(&self) -> Result<elements::BlockHash, Error>;

    async fn get_tx(&self, txid: elements::Txid) -> Result<elements::Transaction, Error>;

    async fn broadcast_tx(&self, signed_tx: &elements::Transaction) -> Result<String, Error>;

    fn network(&self) -> LiquidChain;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression guard for the signet/testnet3 conflation. Signet and testnet3
    /// share an address encoding and BIP32 version bytes, so a wrong mapping
    /// here produces no error at all — just swaps settled on the wrong chain.
    #[test]
    fn signet_resolves_to_signet_not_testnet3() {
        assert_eq!(
            BitcoinChain::from(Network::Signet),
            BitcoinChain::BitcoinSignet
        );
        assert_eq!(
            bitcoin::Network::from(BitcoinChain::from(Network::Signet)),
            bitcoin::Network::Signet
        );
        assert_eq!(
            bitcoin::Network::from(Network::Signet),
            bitcoin::Network::Signet
        );
        // Liquid has no signet, so signet BTC pairs with Liquid testnet.
        assert_eq!(
            LiquidChain::from(Network::Signet),
            LiquidChain::LiquidTestnet
        );
        // ...and testnet must not have been quietly repointed at signet.
        assert_eq!(
            BitcoinChain::from(Network::Testnet),
            BitcoinChain::BitcoinTestnet
        );
        assert_eq!(
            bitcoin::Network::from(Network::Testnet),
            bitcoin::Network::Testnet
        );
    }

    #[test]
    fn currency_defaults_preserve_legacy_chain_strings() {
        assert_eq!(
            Chain::Bitcoin(BitcoinChain::BitcoinRegtest)
                .resolve_currency(None)
                .unwrap(),
            Currency::Btc
        );
        assert_eq!(
            Chain::Liquid(LiquidChain::LiquidRegtest)
                .resolve_currency(None)
                .unwrap(),
            Currency::LBtc
        );
    }

    #[test]
    fn currency_validation_separates_asset_from_network() {
        let bitcoin = Chain::Bitcoin(BitcoinChain::BitcoinRegtest);
        let liquid = Chain::Liquid(LiquidChain::LiquidRegtest);

        assert_eq!(
            liquid.resolve_currency(Some(Currency::LUsdt)).unwrap(),
            Currency::LUsdt
        );
        assert!(bitcoin.resolve_currency(Some(Currency::LUsdt)).is_err());
        assert!(liquid.resolve_currency(Some(Currency::Btc)).is_err());
        assert_eq!(Currency::LUsdt.to_string(), "L-USDT");
    }
}
