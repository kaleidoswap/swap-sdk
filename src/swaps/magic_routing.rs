use std::str::FromStr;

use super::boltz::{BoltzApiClientV2, MrhResponse};
use crate::network::LiquidChain;
use crate::{error::Error, network::Chain};
use bitcoin::{
    hashes::{sha256, Hash},
    hex::FromHex,
    key::{Keypair, Secp256k1},
    secp256k1::{schnorr::Signature, Message},
    PublicKey,
};
use lightning_invoice::{Bolt11Invoice, RouteHintHop};
use serde::{Deserialize, Serialize};

const MAGIC_ROUTING_HINT_CONSTANT: u64 = 596385002596073472;
pub type Bip21Components = (String, String, bitcoin::Amount, Option<String>);

/// Decodes the provided invoice to find the magic routing hint.
pub fn find_magic_routing_hint(invoice: &str) -> Result<Option<RouteHintHop>, Error> {
    Ok(find_magic_routing_hint_in(&Bolt11Invoice::from_str(
        invoice,
    )?))
}

fn find_magic_routing_hint_in(invoice: &Bolt11Invoice) -> Option<RouteHintHop> {
    invoice
        .private_routes()
        .iter()
        .flat_map(|route| &route.0)
        .find(|hint| hint.short_channel_id == MAGIC_ROUTING_HINT_CONSTANT)
        .cloned()
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

/// A magic-routing destination that passed every check in
/// [`verify_mrh_response`]: the maker's signature over the address, the
/// BIP21 scheme, address network and asset against the swap's chain, and the
/// amount against the invoice. Paying `amount` to `address` on-chain settles
/// the invoice without a swap.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MrhDestination {
    pub address: String,
    pub amount_sat: u64,
}

/// Check for magic routing hint in invoice. If present, get the BIP21 from Boltz and verify it.
/// Returns the BIP21 (address, amount) tupple.
pub async fn check_for_mrh(
    boltz_api_v2: &BoltzApiClientV2,
    invoice: &str,
    network: Chain,
) -> Result<Option<(String, bitcoin::Amount)>, Error> {
    Ok(check_mrh(boltz_api_v2, invoice, network)
        .await?
        .map(|destination| {
            (
                destination.address,
                bitcoin::Amount::from_sat(destination.amount_sat),
            )
        }))
}

/// The validated magic-routing destination for `invoice` on `network`, or
/// `None` when the invoice carries no hint. Fetches the maker's BIP21 and
/// runs it through [`verify_mrh_response`]; the bindings expose this so no
/// caller has to validate a raw [`MrhResponse`] itself.
pub async fn check_mrh(
    boltz_api_v2: &BoltzApiClientV2,
    invoice: &str,
    network: Chain,
) -> Result<Option<MrhDestination>, Error> {
    let parsed = Bolt11Invoice::from_str(invoice)?;
    if find_magic_routing_hint_in(&parsed).is_none() {
        return Ok(None);
    }
    let mrh_resp = boltz_api_v2.get_mrh_bip21(invoice).await?;
    verify_mrh_response_parsed(&parsed, &mrh_resp, network).map(Some)
}

/// Verify a maker's `GET /swap/reverse/{invoice}/bip21` response against the
/// invoice it was fetched for and the chain the caller intends to pay on.
///
/// The maker signs only the address, so everything else in the BIP21 is
/// unauthenticated and is checked here: the scheme and address must belong to
/// `network`, the asset must be the chain's bitcoin, and the amount must be
/// positive and at most the invoice amount. It may legitimately be smaller
/// (the maker deducts its service and on-chain fees), never larger: an
/// inflated amount would make an obedient sender overpay a signed address.
/// An invoice without an amount cannot bound the payment and is refused.
///
/// Errors when the invoice carries no magic routing hint, since then the
/// response has no route hint to be signed by.
pub fn verify_mrh_response(
    invoice: &str,
    response: &MrhResponse,
    network: Chain,
) -> Result<MrhDestination, Error> {
    verify_mrh_response_parsed(&Bolt11Invoice::from_str(invoice)?, response, network)
}

fn verify_mrh_response_parsed(
    invoice: &Bolt11Invoice,
    response: &MrhResponse,
    network: Chain,
) -> Result<MrhDestination, Error> {
    let route_hint = find_magic_routing_hint_in(invoice)
        .ok_or_else(|| Error::Protocol("Invoice carries no magic routing hint".to_string()))?;
    let (bip21_network, address, amount, assetid) = verify_mrh_signature(
        &response.bip21,
        &route_hint.src_node_id.to_string(),
        &response.signature,
    )?;
    validate_mrh_destination(
        &bip21_network,
        &address,
        amount,
        assetid.as_deref(),
        network,
    )?;
    validate_mrh_amount(invoice, amount)?;
    Ok(MrhDestination {
        address,
        amount_sat: amount.to_sat(),
    })
}

/// The BIP21 amount is outside the maker's signature. Bound it by the invoice:
/// zero is refused, and so is anything above the invoice amount. Smaller is
/// fine, since the maker takes its fees out of the on-chain leg.
fn validate_mrh_amount(invoice: &Bolt11Invoice, amount: bitcoin::Amount) -> Result<(), Error> {
    let invoice_msat = invoice.amount_milli_satoshis().ok_or_else(|| {
        Error::Protocol(
            "Magic Routing Hint requires an invoice with an amount to bound the BIP21 amount"
                .to_string(),
        )
    })?;
    let bip21_msat = amount.to_sat().checked_mul(1_000).ok_or_else(|| {
        Error::Protocol("Magic Routing Hint amount does not fit in millisatoshis".to_string())
    })?;
    if bip21_msat == 0 {
        return Err(Error::Protocol(
            "Magic Routing Hint amount must be greater than zero".to_string(),
        ));
    }
    if bip21_msat > invoice_msat {
        return Err(Error::Protocol(format!(
            "Magic Routing Hint amount exceeds the invoice amount: {bip21_msat} msat > {invoice_msat} msat"
        )));
    }
    Ok(())
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
            let expected_network: bitcoin::Network = bitcoin_chain.into();
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

    use bitcoin::hashes::{sha256, Hash};

    use crate::network::{BitcoinChain, Chain, LiquidChain};
    use crate::swaps::boltz::MrhResponse;
    use crate::swaps::magic_routing::{
        find_magic_routing_hint, parse_bip21, sign_address, validate_mrh_destination,
        verify_mrh_response, verify_mrh_signature, MrhDestination, MAGIC_ROUTING_HINT_CONSTANT,
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

    /// A regtest invoice signed by a throwaway node key, carrying the magic
    /// routing hint from `maker` and `amount_msat` if given.
    fn mrh_invoice(maker: &Keypair, amount_msat: Option<u64>) -> String {
        use lightning_invoice::{
            Currency, InvoiceBuilder, PaymentSecret, RouteHint, RouteHintHop, RoutingFees,
        };

        let secp = Secp256k1::new();
        let node_key = SecretKey::from_slice(&[41u8; 32]).expect("valid test secret");
        let hint = RouteHint(vec![RouteHintHop {
            src_node_id: maker.public_key(),
            short_channel_id: MAGIC_ROUTING_HINT_CONSTANT,
            fees: RoutingFees {
                base_msat: 0,
                proportional_millionths: 0,
            },
            cltv_expiry_delta: 80,
            htlc_minimum_msat: None,
            htlc_maximum_msat: None,
        }]);
        let mut builder = InvoiceBuilder::new(Currency::Regtest)
            .description("magic routing hint test".to_string())
            .payment_hash(sha256::Hash::hash(&[9u8; 32]))
            .payment_secret(PaymentSecret([7u8; 32]))
            .duration_since_epoch(std::time::Duration::from_secs(1_726_000_000))
            .min_final_cltv_expiry_delta(144)
            .private_route(hint);
        if let Some(amount_msat) = amount_msat {
            builder = builder.amount_milli_satoshis(amount_msat);
        }
        builder
            .build_signed(|hash| secp.sign_ecdsa_recoverable(hash, &node_key))
            .unwrap()
            .to_string()
    }

    /// The maker's signed BIP21 for `address`, with an unsigned `amount`.
    fn mrh_response(maker: &Keypair, address: &str, amount: bitcoin::Amount) -> MrhResponse {
        MrhResponse {
            bip21: format!(
                "bitcoin:{address}?amount={}",
                amount.to_string_in(bitcoin::Denomination::Bitcoin)
            ),
            signature: sign_address(address, maker).unwrap().to_string(),
        }
    }

    #[macros::test_all]
    fn mrh_amount_is_bounded_by_the_invoice() {
        let secp = Secp256k1::new();
        let maker = Keypair::from_secret_key(
            &secp,
            &SecretKey::from_slice(&[0x42; 32]).expect("valid test secret"),
        );
        let address = bitcoin::Address::p2wpkh(
            &bitcoin::CompressedPublicKey(maker.public_key()),
            bitcoin::Network::Regtest,
        )
        .to_string();
        let chain = Chain::Bitcoin(BitcoinChain::BitcoinRegtest);
        let invoice = mrh_invoice(&maker, Some(100_000_000));

        // The maker deducts its fees from the on-chain leg, so equal and
        // smaller amounts are both how a genuine hint looks.
        for amount_sat in [100_000, 99_000, 1] {
            let response = mrh_response(&maker, &address, bitcoin::Amount::from_sat(amount_sat));
            assert_eq!(
                verify_mrh_response(&invoice, &response, chain).unwrap(),
                MrhDestination {
                    address: address.clone(),
                    amount_sat,
                },
                "amount {amount_sat} should be accepted"
            );
        }

        // The amount is outside the signature: an inflated one must not make
        // a sender overpay a correctly signed address.
        for amount_sat in [100_001, 1_000_000] {
            let response = mrh_response(&maker, &address, bitcoin::Amount::from_sat(amount_sat));
            let err = verify_mrh_response(&invoice, &response, chain).unwrap_err();
            assert!(
                err.to_string().contains("exceeds the invoice amount"),
                "amount {amount_sat} should be rejected, got: {err}"
            );
        }

        let zero = mrh_response(&maker, &address, bitcoin::Amount::ZERO);
        assert!(verify_mrh_response(&invoice, &zero, chain).is_err());
    }

    #[macros::test_all]
    fn mrh_requires_an_invoice_amount_and_a_routing_hint() {
        let secp = Secp256k1::new();
        let maker = Keypair::from_secret_key(
            &secp,
            &SecretKey::from_slice(&[0x42; 32]).expect("valid test secret"),
        );
        let address = bitcoin::Address::p2wpkh(
            &bitcoin::CompressedPublicKey(maker.public_key()),
            bitcoin::Network::Regtest,
        )
        .to_string();
        let chain = Chain::Bitcoin(BitcoinChain::BitcoinRegtest);
        let response = mrh_response(&maker, &address, bitcoin::Amount::from_sat(1_000));

        // Without an invoice amount nothing bounds the BIP21 amount.
        let amountless = mrh_invoice(&maker, None);
        let err = verify_mrh_response(&amountless, &response, chain).unwrap_err();
        assert!(
            err.to_string().contains("invoice with an amount"),
            "unexpected error: {err}"
        );

        // Without a hint there is no maker key the signature could be from.
        let plain = mrh_invoice_without_hint();
        assert!(verify_mrh_response(&plain, &response, chain).is_err());

        // A hint from a different node than the one that signed the address.
        let other = Keypair::from_secret_key(
            &secp,
            &SecretKey::from_slice(&[0x43; 32]).expect("valid test secret"),
        );
        let foreign = mrh_invoice(&other, Some(1_000_000));
        assert!(verify_mrh_response(&foreign, &response, chain).is_err());
    }

    fn mrh_invoice_without_hint() -> String {
        use lightning_invoice::{Currency, InvoiceBuilder, PaymentSecret};

        let secp = Secp256k1::new();
        let node_key = SecretKey::from_slice(&[41u8; 32]).expect("valid test secret");
        InvoiceBuilder::new(Currency::Regtest)
            .description("no hint".to_string())
            .payment_hash(sha256::Hash::hash(&[9u8; 32]))
            .payment_secret(PaymentSecret([7u8; 32]))
            .duration_since_epoch(std::time::Duration::from_secs(1_726_000_000))
            .min_final_cltv_expiry_delta(144)
            .amount_milli_satoshis(1_000_000)
            .build_signed(|hash| secp.sign_ecdsa_recoverable(hash, &node_key))
            .unwrap()
            .to_string()
    }

    #[macros::test_all]
    fn test_mrh() {
        let route_hint = find_magic_routing_hint("lntb1m1pnrv328pp5zymney8y48234em5lakrkuk8rfrftn5dkwfys7zghe2c40hxfmusdpz2djkuepqw3hjqnpdgf2yxgrpv3j8yetnwvcqz95xqyp2xqrzjqwyg6p2yhhqvq5d97kkwuk0mnrp3su6sn5fvtxn63gppms9fkegajzzxeyqq28qqqqqqqqqqqqqqq9gq2ysp5znw62my456pnzq7vyfgje2yjfat8gzgf88q8rl30dt3cgpmpk9eq9qyyssq55qds9y2vrtmqxq00fgrnartdhs0wwlt7u5uflzs5wnx8wad8y3y86y8lgre4qaszhvhesa6ts99g7m088j6dgjfe6hhtkfglqfqwjcp03v2nh").unwrap().expect("route hint expected");
        assert_eq!(route_hint.short_channel_id, MAGIC_ROUTING_HINT_CONSTANT);
    }
}
