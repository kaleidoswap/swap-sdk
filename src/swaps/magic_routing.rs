use std::str::FromStr;

use super::boltz::BoltzApiClientV2;
use crate::network::{BitcoinChain, LiquidChain};
use crate::{error::Error, network::Chain};
use bitcoin::{
    hashes::{sha256, Hash},
    hex::FromHex,
    key::{Keypair, Secp256k1},
    secp256k1::{schnorr::Signature, Message},
    PublicKey,
};
use lightning_invoice::{Bolt11Invoice, RouteHintHop};

const MAGIC_ROUTING_HINT_CONSTANT: u64 = 596385002596073472;
pub type Bip21Components = (String, String, bitcoin::Amount, Option<String>);

/// Decodes the provided invoice to find the magic routing hint.
pub fn find_magic_routing_hint(invoice: &str) -> Result<Option<RouteHintHop>, Error> {
    let invoice = Bolt11Invoice::from_str(invoice)?;
    Ok(invoice
        .private_routes()
        .iter()
        .flat_map(|route| &route.0)
        .find(|hint| hint.short_channel_id == MAGIC_ROUTING_HINT_CONSTANT)
        .cloned())
}

/// Parse a BIP21 String and get the network, address, asset_id if present
pub fn parse_bip21(uri: &str) -> Result<Bip21Components, Error> {
    let (network_address, params) = uri
        .split_once('?')
        .ok_or_else(|| Error::Generic("BIP21 URI must contain a query string".to_string()))?;
    if params.contains('?') {
        return Err(Error::Generic(
            "BIP21 URI contains more than one query separator".to_string(),
        ));
    }

    // Extract network and address
    let (network, address) = network_address.split_once(':').ok_or_else(|| {
        Error::Generic("Unable to extract network and address from BIP21 string".to_string())
    })?;
    if network.is_empty() || address.is_empty() {
        return Err(Error::Generic(
            "BIP21 network and address must not be empty".to_string(),
        ));
    }

    // Parse URI parameters
    let mut amount = bitcoin::Amount::from_sat(0);
    let mut assetid = None::<String>;
    let mut amount_seen = false;

    for param in params.split('&') {
        let (key, value) = param
            .split_once('=')
            .ok_or_else(|| Error::Generic(format!("Malformed BIP21 parameter: {param}")))?;
        if value.is_empty() {
            return Err(Error::Generic(format!(
                "BIP21 parameter {key} must not be empty"
            )));
        }
        match key {
            "amount" => {
                if amount_seen {
                    return Err(Error::Generic(
                        "BIP21 URI contains duplicate amount parameters".to_string(),
                    ));
                }
                amount_seen = true;
                amount = bitcoin::Amount::from_str_in(value, bitcoin::Denomination::Bitcoin)
                    .map_err(|e| {
                        Error::Generic(format!("Unable to parse amount from string: {e}"))
                    })?;
            }
            "assetid" => {
                if assetid.is_some() {
                    return Err(Error::Generic(
                        "BIP21 URI contains duplicate assetid parameters".to_string(),
                    ));
                }
                assetid = Some(value.into());
            }
            _ => {}
        }
    }

    Ok((network.into(), address.into(), amount, assetid))
}

/// Check for magic routing hint in invoice. If present, get the BIP21 from Boltz and verify it.
/// Returns the BIP21 (address, amount) tupple.
pub async fn check_for_mrh(
    boltz_api_v2: &BoltzApiClientV2,
    invoice: &str,
    network: Chain,
) -> Result<Option<(String, bitcoin::Amount)>, Error> {
    if let Some(route_hint) = find_magic_routing_hint(invoice)? {
        let mrh_resp = boltz_api_v2.get_mrh_bip21(invoice).await?;

        let (bip21_network, address, amount, assetid) = verify_mrh_signature(
            &mrh_resp.bip21,
            &route_hint.src_node_id.to_string(),
            &mrh_resp.signature,
        )?;
        validate_mrh_destination(
            &bip21_network,
            &address,
            amount,
            assetid.as_deref(),
            network,
        )?;

        Ok(Some((address, amount)))
    } else {
        Ok(None)
    }
}

fn validate_mrh_destination(
    bip21_network: &str,
    address: &str,
    amount: bitcoin::Amount,
    assetid: Option<&str>,
    chain: Chain,
) -> Result<(), Error> {
    if amount == bitcoin::Amount::ZERO {
        return Err(Error::Protocol(
            "Magic Routing Hint amount must be greater than zero".to_string(),
        ));
    }

    match chain {
        Chain::Liquid(liquid_chain) => {
            let scheme_matches = match liquid_chain {
                LiquidChain::Liquid => bip21_network == "liquidnetwork",
                LiquidChain::LiquidTestnet => bip21_network == "liquidtestnet",
                // Boltz uses `liquidnetwork` for its Elements regtest backend,
                // while Kaleido Maker uses `liquidtestnet`. The address params
                // and policy asset below still unambiguously enforce regtest.
                LiquidChain::LiquidRegtest => {
                    matches!(bip21_network, "liquidnetwork" | "liquidtestnet")
                }
            };
            if !scheme_matches {
                return Err(Error::Protocol(
                    "Network mismatch in Magic Routing Hint".to_string(),
                ));
            }

            let parsed = elements::Address::from_str(address)?;
            let expected_params: &'static elements::AddressParams = liquid_chain.into();
            if parsed.params != expected_params || parsed.to_string() != address {
                return Err(Error::Protocol(
                    "Address network mismatch in Magic Routing Hint".to_string(),
                ));
            }

            let expected_asset = liquid_chain.bitcoin().to_string();
            if assetid != Some(expected_asset.as_str()) {
                return Err(Error::Protocol(
                    "Asset Id mismatch in Magic Routing Hint".to_string(),
                ));
            }
        }
        Chain::Bitcoin(bitcoin_chain) => {
            if bip21_network != "bitcoin" || assetid.is_some() {
                return Err(Error::Protocol(
                    "Network or asset mismatch in Magic Routing Hint".to_string(),
                ));
            }
            let expected_network = match bitcoin_chain {
                BitcoinChain::Bitcoin => bitcoin::Network::Bitcoin,
                BitcoinChain::BitcoinTestnet => bitcoin::Network::Testnet,
                BitcoinChain::BitcoinRegtest => bitcoin::Network::Regtest,
            };
            bitcoin::Address::from_str(address)?.require_network(expected_network)?;
        }
    }

    Ok(())
}

pub fn verify_mrh_signature(
    bip21: &str,
    pubkey: &str,
    signature: &str,
) -> Result<Bip21Components, Error> {
    let (network, address, amount, assetid) = parse_bip21(bip21)?;
    let address_hash = sha256::Hash::hash(address.as_bytes());
    let msg = Message::from_digest_slice(address_hash.as_byte_array())?;

    let receiver_sig = Signature::from_slice(&Vec::from_hex(signature)?)?;

    let receiver_pubkey = PublicKey::from_str(pubkey)?.inner;

    let secp = Secp256k1::new();
    secp.verify_schnorr(&receiver_sig, &msg, &receiver_pubkey.x_only_public_key().0)?;

    Ok((network, address, amount, assetid))
}

/// Sign the address signature by a priv key.
pub fn sign_address(addr: &str, keys: &Keypair) -> Result<Signature, Error> {
    let address_hash = sha256::Hash::hash(addr.as_bytes());
    let msg = Message::from_digest_slice(address_hash.as_byte_array())?;
    Ok(Secp256k1::new().sign_schnorr(&msg, keys))
}

#[cfg(test)]
mod tests {
    use bitcoin::key::Keypair;
    use bitcoin::secp256k1::{Secp256k1, SecretKey};

    use crate::network::{Chain, LiquidChain};
    use crate::swaps::magic_routing::{
        find_magic_routing_hint, parse_bip21, sign_address, validate_mrh_destination,
        verify_mrh_signature, MAGIC_ROUTING_HINT_CONSTANT,
    };

    #[macros::test_all]
    fn test_bip21_parsing() {
        let uri = "liquidtestnet:tlq1qqt3sgky7zert7237tred5rqmmx0eargp625zkyhr2ldw6yqdvh5fusnm5xk0qfjpejvgm37q7mqtv5epfksv78jweytmqgpd8?amount=0.00005122&assetid=144c654344aa716d6f3abcc1ca90e5641e4e2a7f633bc09fe3baf64585819a4";
        let (network, address, amount, assetid) = parse_bip21(uri).unwrap();

        assert_eq!(network, "liquidtestnet");
        assert_eq!(address, "tlq1qqt3sgky7zert7237tred5rqmmx0eargp625zkyhr2ldw6yqdvh5fusnm5xk0qfjpejvgm37q7mqtv5epfksv78jweytmqgpd8");
        assert_eq!(amount.to_btc(), 0.00005122);
        assert_eq!(
            assetid,
            Some("144c654344aa716d6f3abcc1ca90e5641e4e2a7f633bc09fe3baf64585819a4".to_string())
        );
    }

    /// BIP21 amounts which can lead to rounding errors when converting from BTC amount (f64) to sats (u64).
    /// The format is: (sat amount, BIP21 BTC amount)
    fn get_bip21_rounding_test_vectors() -> Vec<(u64, f64)> {
        vec![
            (999, 0.0000_0999),
            (1_000, 0.0000_1000),
            (59_810, 0.0005_9810),
        ]
    }

    #[macros::test_all]
    fn test_bip21_parsing_with_rounding_edge_cases() {
        let liquid_address = "tlq1qqt3sgky7zert7237tred5rqmmx0eargp625zkyhr2ldw6yqdvh5fusnm5xk0qfjpejvgm37q7mqtv5epfksv78jweytmqgpd8";
        let asset_id = "144c654344aa716d6f3abcc1ca90e5641e4e2a7f633bc09fe3baf64585819a4";

        for (amount_sat, amount_btc) in get_bip21_rounding_test_vectors() {
            let uri =
                format!("liquidtestnet:{liquid_address}?amount={amount_btc}&assetid={asset_id}");
            let (_network, _address, bip21_amount, _assetid) = parse_bip21(&uri).unwrap();

            let parsed_amount_sat = bip21_amount.to_sat();

            assert_eq!(parsed_amount_sat, amount_sat);
        }
    }

    #[macros::test_all]
    fn malformed_bip21_is_rejected_without_panicking() {
        for uri in [
            "liquidtestnet:ert1invalid",
            "?amount=0.1",
            "liquidtestnet:?amount=0.1",
            "liquidtestnet:ert1invalid?amount",
            "liquidtestnet:ert1invalid?amount=",
            "liquidtestnet:ert1invalid?amount=0.1&amount=0.2",
            "liquidtestnet:ert1invalid?amount=0.1?assetid=00",
        ] {
            assert!(parse_bip21(uri).is_err(), "accepted malformed URI: {uri}");
        }
    }

    #[macros::test_all]
    fn mrh_signature_rejects_address_tampering() {
        let secp = Secp256k1::new();
        let keypair = Keypair::from_secret_key(
            &secp,
            &SecretKey::from_slice(&[0x42; 32]).expect("valid test secret"),
        );
        let address = "ert1psefavkmha2udzsdkm7cqvq9kyp7pl077meesm3m29qygs4nef0vqcgyqml";
        let signature = sign_address(address, &keypair).unwrap();
        let bip21 = format!(
            "liquidtestnet:{address}?amount=0.00001000&assetid={}",
            LiquidChain::LiquidRegtest.bitcoin()
        );

        verify_mrh_signature(
            &bip21,
            &keypair.public_key().to_string(),
            &signature.to_string(),
        )
        .unwrap();

        let tampered = bip21.replace(
            address,
            "ert1pqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq",
        );
        assert!(verify_mrh_signature(
            &tampered,
            &keypair.public_key().to_string(),
            &signature.to_string(),
        )
        .is_err());
    }

    #[macros::test_all]
    fn mrh_destination_validates_liquid_regtest_network_asset_and_amount() {
        // Upstream Boltz emits a `liquidnetwork` URI with an Elements regtest
        // address, while Kaleido Maker emits `liquidtestnet` for the same chain.
        let address = "el1pqtl0qtngg839weftqsjq5kplk4aeq52l30z0pawjamw4vp8peyuswfuatpvq3emule96g7pkun4jl0u7mtausxaquzlqt2rcsqearadvq2vm50jvcl0j";
        let asset = LiquidChain::LiquidRegtest.bitcoin().to_string();
        let chain = Chain::Liquid(LiquidChain::LiquidRegtest);
        let amount = bitcoin::Amount::from_sat(1_000);

        validate_mrh_destination("liquidtestnet", address, amount, Some(&asset), chain).unwrap();
        validate_mrh_destination("liquidnetwork", address, amount, Some(&asset), chain).unwrap();
        assert!(validate_mrh_destination("bitcoin", address, amount, Some(&asset), chain).is_err());
        assert!(validate_mrh_destination(
            "liquidtestnet",
            address,
            amount,
            Some(&LiquidChain::LiquidTestnet.bitcoin().to_string()),
            chain,
        )
        .is_err());
        assert!(validate_mrh_destination(
            "liquidtestnet",
            address,
            bitcoin::Amount::ZERO,
            Some(&asset),
            chain,
        )
        .is_err());
    }

    #[macros::test_all]
    fn test_mrh() {
        let route_hint = find_magic_routing_hint("lntb1m1pnrv328pp5zymney8y48234em5lakrkuk8rfrftn5dkwfys7zghe2c40hxfmusdpz2djkuepqw3hjqnpdgf2yxgrpv3j8yetnwvcqz95xqyp2xqrzjqwyg6p2yhhqvq5d97kkwuk0mnrp3su6sn5fvtxn63gppms9fkegajzzxeyqq28qqqqqqqqqqqqqqq9gq2ysp5znw62my456pnzq7vyfgje2yjfat8gzgf88q8rl30dt3cgpmpk9eq9qyyssq55qds9y2vrtmqxq00fgrnartdhs0wwlt7u5uflzs5wnx8wad8y3y86y8lgre4qaszhvhesa6ts99g7m088j6dgjfe6hhtkfglqfqwjcp03v2nh").unwrap().expect("route hint expected");
        assert_eq!(route_hint.short_channel_id, MAGIC_ROUTING_HINT_CONSTANT);
    }
}
