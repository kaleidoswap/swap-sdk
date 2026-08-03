use kaleidorg_swap_sdk::network::electrum::{ElectrumBitcoinClient, ElectrumLiquidClient};
use kaleidorg_swap_sdk::network::esplora::{EsploraBitcoinClient, EsploraLiquidClient};
use kaleidorg_swap_sdk::network::{BitcoinChain, Chain, Currency, LiquidChain, Network};
use kaleidorg_swap_sdk::swaps::ChainClient as CoreClient;

use crate::boltz::Error;
use crate::swap::BtcLikeTransaction;

// NB: variant order here is the FFI wire order, independent of the core enum's
// declaration order. New variants go at the END so existing indices keep their
// meaning — inserting mid-enum would make a stale generated module decode
// `Mainnet` as `Signet` against a freshly built library.
#[uniffi::remote(Enum)]
pub enum Network {
    Regtest,
    Testnet,
    Mainnet,
    Signet,
}

#[uniffi::remote(Enum)]
pub enum LiquidChain {
    Liquid,
    LiquidTestnet,
    LiquidRegtest,
}

// Appended, not inserted — see the note on `Network` above.
#[uniffi::remote(Enum)]
pub enum BitcoinChain {
    Bitcoin,
    BitcoinTestnet,
    BitcoinRegtest,
    BitcoinSignet,
}

#[uniffi::remote(Enum)]
pub enum Chain {
    Bitcoin(BitcoinChain),
    Liquid(LiquidChain),
}

#[uniffi::remote(Enum)]
pub enum Currency {
    Btc,
    LBtc,
    LUsdt,
}

#[derive(uniffi::Record)]
pub struct EsploraBuilder {
    pub url: String,
    #[uniffi(default = 30)]
    pub timeout: u64,
}

#[derive(uniffi::Record)]
pub struct ElectrumBuilder {
    pub url: String,
    pub tls: bool,
    #[uniffi(default = 10)]
    pub timeout: u8,
    #[uniffi(default = true)]
    pub validate_domain: bool,
}

#[derive(uniffi::Enum)]
pub enum ClientConnection {
    Esplora(EsploraBuilder),
    Electrum(ElectrumBuilder),
}

#[derive(uniffi::Record)]
pub struct ClientConfig {
    pub network: Network,
    pub bitcoin: Option<ClientConnection>,
    pub liquid: Option<ClientConnection>,
}

#[uniffi::export]
pub fn btc_chain_from_network(network: Network) -> Chain {
    let btc_chain: BitcoinChain = network.into();
    btc_chain.into()
}

#[uniffi::export]
pub fn lbtc_chain_from_network(network: Network) -> Chain {
    let lbtc_chain: LiquidChain = network.into();
    lbtc_chain.into()
}

#[derive(uniffi::Object)]
pub struct ChainClient(pub(crate) CoreClient);

#[uniffi::export]
impl ChainClient {
    #[uniffi::constructor]
    pub fn new(config: ClientConfig) -> Result<Self, Error> {
        let mut client = CoreClient::new();
        if let Some(connection) = config.bitcoin {
            client = match connection {
                ClientConnection::Esplora(esplora) => client.with_bitcoin(
                    EsploraBitcoinClient::new(config.network.into(), &esplora.url, esplora.timeout),
                ),
                ClientConnection::Electrum(electrum) => client.with_bitcoin(
                    ElectrumBitcoinClient::new(
                        config.network.into(),
                        &electrum.url,
                        electrum.tls,
                        electrum.validate_domain,
                        electrum.timeout,
                    )
                    .map_err(|e| Error::Generic(e.to_string()))?,
                ),
            };
        };
        if let Some(connection) = config.liquid {
            client = match connection {
                ClientConnection::Esplora(esplora) => client.with_liquid(EsploraLiquidClient::new(
                    config.network.into(),
                    &esplora.url,
                    esplora.timeout,
                )),
                ClientConnection::Electrum(electrum) => client.with_liquid(
                    ElectrumLiquidClient::new(
                        config.network.into(),
                        &electrum.url,
                        electrum.tls,
                        electrum.validate_domain,
                        electrum.timeout,
                    )
                    .map_err(|e| Error::Generic(e.to_string()))?,
                ),
            };
        };
        Ok(ChainClient(client))
    }
}

#[uniffi::export]
impl ChainClient {
    #[uniffi::method]
    pub async fn broadcast_tx(&self, tx: &BtcLikeTransaction) -> Result<String, Error> {
        self.0.broadcast_tx(&tx.0).await.map_err(|e| e.into())
    }
}
