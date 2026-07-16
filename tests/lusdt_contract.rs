use std::str::FromStr;

use bitcoin::hashes::{ripemd160, Hash};
use elements::{
    opcodes::all::{OP_CHECKSIG, OP_CHECKSIGVERIFY, OP_CLTV, OP_EQUALVERIFY, OP_HASH160, OP_SIZE},
    script::Builder,
    secp256k1_zkp::{PublicKey, Secp256k1, SecretKey},
    taproot::{LeafVersion, TaprootBuilder},
    Address, AddressParams, Script,
};
use lightning_invoice::Bolt11Invoice;
use secp256k1_musig::musig;
use serde_json::Value;

use kaleidoswap_sdk::network::{Chain, Currency, LiquidChain};
use kaleidoswap_sdk::swaps::boltz::{
    CreateChainResponse, CreateReverseResponse, CreateSubmarineResponse, GetChainPairsResponse,
    GetReversePairsResponse, GetSubmarinePairsResponse, Side,
};
use kaleidoswap_sdk::LiquidSwapScript;

const GOLDEN: &str = include_str!("fixtures/lusdt-v1/liquid-golden-vectors.json");
const WIRE: &str = include_str!("fixtures/lusdt-v1/wire-contract.json");

fn value_at<'a>(value: &'a Value, pointer: &str) -> &'a Value {
    value
        .pointer(pointer)
        .unwrap_or_else(|| panic!("fixture path is required: {pointer}"))
}

fn string_at<'a>(value: &'a Value, pointer: &str) -> &'a str {
    value_at(value, pointer)
        .as_str()
        .unwrap_or_else(|| panic!("fixture path must be a string: {pointer}"))
}

fn assert_asset_id(value: &Value, pointer: &str, expected: &str) {
    let asset = string_at(value, pointer);
    assert_eq!(asset, expected, "wrong asset at {pointer}");
    assert_eq!(hex::decode(asset).expect("asset id must be hex").len(), 32);
}

fn assert_exactly_one_amount(request: &Value) {
    let user = request.get("userLockAmount").is_some();
    let server = request.get("serverLockAmount").is_some();
    assert_ne!(
        user, server,
        "chain requests require exactly one lock amount"
    );
}

#[test]
fn wire_fixture_freezes_the_lusdt_v1_contract() {
    let fixture: Value = serde_json::from_str(WIRE).expect("valid wire fixture");
    assert_eq!(value_at(&fixture, "/schemaVersion"), 1);

    let lusdt = string_at(&fixture, "/assets/lusdt");
    let policy = string_at(&fixture, "/assets/policy");
    assert_ne!(lusdt, policy, "L-USDT is not the Elements policy asset");

    assert_eq!(
        string_at(&fixture, "/amountSemantics/limits"),
        "inputAssetBaseUnits"
    );
    assert_eq!(
        string_at(&fixture, "/amountSemantics/rate"),
        "outputAssetBaseUnitsPerInputAssetBaseUnit"
    );
    assert_eq!(
        string_at(&fixture, "/amountSemantics/quotedFees"),
        "outputAssetBaseUnits"
    );
    assert_eq!(
        string_at(&fixture, "/amountSemantics/elementsFeeOutput"),
        "feeAssetId"
    );

    assert_asset_id(
        &fixture,
        "/pairResponses/submarine/L-USDT/BTC/fromAssetId",
        lusdt,
    );
    assert_asset_id(
        &fixture,
        "/pairResponses/submarine/L-USDT/BTC/feeAssetId",
        policy,
    );
    assert!(value_at(
        &fixture,
        "/pairResponses/submarine/L-USDT/BTC/fees/minerFees"
    )
    .is_number());

    assert_asset_id(
        &fixture,
        "/pairResponses/reverse/BTC/L-USDT/toAssetId",
        lusdt,
    );
    assert_asset_id(
        &fixture,
        "/pairResponses/reverse/BTC/L-USDT/feeAssetId",
        policy,
    );
    assert!(value_at(
        &fixture,
        "/pairResponses/reverse/BTC/L-USDT/fees/minerFees/lockup"
    )
    .is_number());
    assert!(value_at(
        &fixture,
        "/pairResponses/reverse/BTC/L-USDT/fees/minerFees/claim"
    )
    .is_number());

    assert_asset_id(&fixture, "/pairResponses/chain/BTC/L-USDT/toAssetId", lusdt);
    assert_asset_id(
        &fixture,
        "/pairResponses/chain/BTC/L-USDT/feeAssetId",
        policy,
    );
    assert!(value_at(
        &fixture,
        "/pairResponses/chain/BTC/L-USDT/fees/minerFees/server"
    )
    .is_number());
    assert!(value_at(
        &fixture,
        "/pairResponses/chain/BTC/L-USDT/fees/minerFees/user/lockup"
    )
    .is_number());
    assert!(value_at(
        &fixture,
        "/pairResponses/chain/BTC/L-USDT/fees/minerFees/user/claim"
    )
    .is_number());

    let submarine_request = value_at(&fixture, "/create/submarine/request");
    assert_eq!(string_at(submarine_request, "/from"), "L-USDT");
    assert_eq!(string_at(submarine_request, "/to"), "BTC");
    assert!(submarine_request.get("fromAmount").is_none());
    let invoice = Bolt11Invoice::from_str(string_at(submarine_request, "/invoice"))
        .expect("fixture BOLT11 invoice must be valid");
    assert_eq!(invoice.amount_milli_satoshis(), Some(100_000_000));
    let invoice_sat = invoice.amount_milli_satoshis().unwrap() / 1_000;
    let service_fee_sat = invoice_sat * 5 / 1_000;
    let miner_fee_sat = value_at(
        &fixture,
        "/pairResponses/submarine/L-USDT/BTC/fees/minerFees",
    )
    .as_u64()
    .unwrap();
    let rate = value_at(&fixture, "/pairResponses/submarine/L-USDT/BTC/rate")
        .as_f64()
        .unwrap();
    let expected_input =
        ((invoice_sat + service_fee_sat + miner_fee_sat) as f64 / rate).ceil() as u64;
    assert_eq!(
        value_at(&fixture, "/create/submarine/response/expectedAmount").as_u64(),
        Some(expected_input)
    );

    for pointer in [
        "/create/chain/userAmountRequest",
        "/create/chain/serverAmountRequest",
    ] {
        let request = value_at(&fixture, pointer);
        assert_exactly_one_amount(request);
        assert!(request.get("userAddress").is_none());
    }

    let submarine_response = value_at(&fixture, "/create/submarine/response");
    assert!(submarine_response.get("blindingKey").is_none());
    assert!(submarine_response
        .get("referralId")
        .is_some_and(Value::is_null));
    assert_asset_id(submarine_response, "/assetId", lusdt);
    assert_asset_id(submarine_response, "/feeAssetId", policy);

    let reverse_response = value_at(&fixture, "/create/reverse/response");
    assert!(reverse_response
        .get("blindingKey")
        .is_some_and(Value::is_null));
    assert_asset_id(reverse_response, "/assetId", lusdt);
    assert_asset_id(reverse_response, "/feeAssetId", policy);

    let bitcoin_lock = value_at(&fixture, "/create/chain/response/lockupDetails");
    assert!(bitcoin_lock.get("assetId").is_none());
    assert!(bitcoin_lock.get("blindingKey").is_none());
    let liquid_lock = value_at(&fixture, "/create/chain/response/claimDetails");
    assert!(liquid_lock.get("blindingKey").is_some_and(Value::is_null));
    assert_asset_id(liquid_lock, "/assetId", lusdt);
    assert_asset_id(liquid_lock, "/feeAssetId", policy);

    for pointer in [
        "/transactionLookups/submarine/id",
        "/transactionLookups/reverse/id",
        "/transactionLookups/chain/userLock/transaction/id",
        "/transactionLookups/chain/serverLock/transaction/id",
    ] {
        assert_eq!(
            hex::decode(string_at(&fixture, pointer))
                .expect("txid hex")
                .len(),
            32
        );
    }
    assert_eq!(
        string_at(&fixture, "/transactionLookups/transactionNotFound/error"),
        "transaction_not_found"
    );

    let statuses = value_at(&fixture, "/websocketStatusSequence")
        .as_array()
        .expect("status sequence array")
        .iter()
        .map(|status| status.as_str().expect("status string"))
        .collect::<Vec<_>>();
    assert_eq!(
        statuses,
        [
            "swap.created",
            "transaction.mempool",
            "transaction.confirmed",
            "transaction.server.mempool",
            "transaction.server.confirmed",
            "transaction.claimed",
        ]
    );
}

#[test]
fn sdk_parses_lusdt_pair_cards_and_asset_extensions() {
    let fixture: Value = serde_json::from_str(WIRE).unwrap();

    let submarine: GetSubmarinePairsResponse = serde_json::from_value(serde_json::json!({
        "BTC": {},
        "L-BTC": {},
        "L-USDT": value_at(&fixture, "/pairResponses/submarine/L-USDT")
    }))
    .unwrap();
    let reverse: GetReversePairsResponse =
        serde_json::from_value(value_at(&fixture, "/pairResponses/reverse").clone()).unwrap();
    let chain: GetChainPairsResponse = serde_json::from_value(serde_json::json!({
        "BTC": value_at(&fixture, "/pairResponses/chain/BTC"),
        "L-BTC": {}
    }))
    .unwrap();

    let submarine_pair = submarine.get_lusdt_to_btc_pair().unwrap();
    assert_eq!(
        submarine_pair.from_asset_id.as_deref(),
        Some(string_at(&fixture, "/assets/lusdt"))
    );
    assert_eq!(
        submarine_pair.fee_asset_id.as_deref(),
        Some(string_at(&fixture, "/assets/policy"))
    );
    assert_eq!(
        reverse
            .get_btc_to_lusdt_pair()
            .unwrap()
            .to_asset_id
            .as_deref(),
        Some(string_at(&fixture, "/assets/lusdt"))
    );
    assert_eq!(
        chain
            .get_btc_to_lusdt_pair()
            .unwrap()
            .fee_asset_id
            .as_deref(),
        Some(string_at(&fixture, "/assets/policy"))
    );
}

#[test]
fn sdk_constructs_explicit_lusdt_scripts_from_frozen_responses() {
    let fixture: Value = serde_json::from_str(WIRE).unwrap();
    let submarine: CreateSubmarineResponse =
        serde_json::from_value(value_at(&fixture, "/create/submarine/response").clone()).unwrap();
    let submarine_refund_key = bitcoin::PublicKey::from_str(string_at(
        &fixture,
        "/create/submarine/request/refundPublicKey",
    ))
    .unwrap();
    let submarine_script =
        LiquidSwapScript::submarine_from_swap_resp(&submarine, submarine_refund_key).unwrap();
    submarine
        .validate_with_currency(
            string_at(&fixture, "/create/submarine/request/invoice"),
            &submarine_refund_key,
            Chain::Liquid(LiquidChain::LiquidRegtest),
            Some(Currency::LUsdt),
        )
        .unwrap();
    submarine_script
        .validate_address(LiquidChain::LiquidRegtest, submarine.address.clone())
        .unwrap();
    assert!(submarine_script.blinding_key.is_none());
    assert!(submarine_script.requires_caller_funded_pset());
    assert_eq!(submarine_script.expected_amount, submarine.expected_amount);

    let mut missing_assets = submarine.clone();
    missing_assets.asset_id = None;
    missing_assets.fee_asset_id = None;
    assert!(missing_assets
        .validate_with_currency(
            string_at(&fixture, "/create/submarine/request/invoice"),
            &submarine_refund_key,
            Chain::Liquid(LiquidChain::LiquidRegtest),
            Some(Currency::LUsdt),
        )
        .is_err());

    let reverse: CreateReverseResponse =
        serde_json::from_value(value_at(&fixture, "/create/reverse/response").clone()).unwrap();
    let reverse_claim_key = bitcoin::PublicKey::from_str(string_at(
        &fixture,
        "/create/reverse/request/claimPublicKey",
    ))
    .unwrap();
    let reverse_script =
        LiquidSwapScript::reverse_from_swap_resp(&reverse, reverse_claim_key).unwrap();
    reverse_script
        .validate_address(LiquidChain::LiquidRegtest, reverse.lockup_address.clone())
        .unwrap();
    assert!(reverse_script.blinding_key.is_none());

    let chain: CreateChainResponse =
        serde_json::from_value(value_at(&fixture, "/create/chain/response").clone()).unwrap();
    let chain_claim_key = bitcoin::PublicKey::from_str(string_at(
        &fixture,
        "/create/chain/userAmountRequest/claimPublicKey",
    ))
    .unwrap();
    let chain_script = LiquidSwapScript::chain_from_swap_resp(
        Side::Claim,
        chain.claim_details.clone(),
        chain_claim_key,
    )
    .unwrap();
    chain_script
        .validate_address(
            LiquidChain::LiquidRegtest,
            chain.claim_details.lockup_address,
        )
        .unwrap();
    assert!(chain_script.requires_caller_funded_pset());
}

#[test]
fn sdk_rejects_noncanonical_liquid_tree_bytes_and_leaf_versions() {
    let fixture: Value = serde_json::from_str(WIRE).unwrap();
    let refund_key = bitcoin::PublicKey::from_str(string_at(
        &fixture,
        "/create/submarine/request/refundPublicKey",
    ))
    .unwrap();

    let mut trailing_opcode: CreateSubmarineResponse =
        serde_json::from_value(value_at(&fixture, "/create/submarine/response").clone()).unwrap();
    trailing_opcode.swap_tree.claim_leaf.output.push_str("00");
    assert!(LiquidSwapScript::submarine_from_swap_resp(&trailing_opcode, refund_key).is_err());

    let mut wrong_version: CreateSubmarineResponse =
        serde_json::from_value(value_at(&fixture, "/create/submarine/response").clone()).unwrap();
    wrong_version.swap_tree.refund_leaf.version = 0xc0;
    assert!(LiquidSwapScript::submarine_from_swap_resp(&wrong_version, refund_key).is_err());
}

fn pubkey_from_secret(secret: &str) -> PublicKey {
    let secp = Secp256k1::new();
    let secret = SecretKey::from_str(secret).expect("valid fixture secret key");
    PublicKey::from_secret_key(&secp, &secret)
}

fn scripts(
    case_name: &str,
    hashlock: ripemd160::Hash,
    claim: PublicKey,
    refund: PublicKey,
    timeout: i64,
) -> (Script, Script) {
    let claim_xonly = claim.x_only_public_key().0.serialize();
    let claim_script = match case_name {
        "submarine" => Builder::new()
            .push_opcode(OP_HASH160)
            .push_slice(hashlock.as_byte_array())
            .push_opcode(OP_EQUALVERIFY)
            .push_slice(&claim_xonly)
            .push_opcode(OP_CHECKSIG)
            .into_script(),
        "reverse" => Builder::new()
            .push_opcode(OP_SIZE)
            .push_int(32)
            .push_opcode(OP_EQUALVERIFY)
            .push_opcode(OP_HASH160)
            .push_slice(hashlock.as_byte_array())
            .push_opcode(OP_EQUALVERIFY)
            .push_slice(&claim_xonly)
            .push_opcode(OP_CHECKSIG)
            .into_script(),
        _ => panic!("unknown golden-vector case"),
    };
    let refund_script = Builder::new()
        .push_slice(&refund.x_only_public_key().0.serialize())
        .push_opcode(OP_CHECKSIGVERIFY)
        .push_int(timeout)
        .push_opcode(OP_CLTV)
        .into_script();
    (claim_script, refund_script)
}

fn musig_internal_key(
    first: PublicKey,
    second: PublicKey,
) -> (musig::KeyAggCache, bitcoin::XOnlyPublicKey) {
    let first = secp256k1_musig::PublicKey::from_slice(&first.serialize()).expect("musig pubkey");
    let second = secp256k1_musig::PublicKey::from_slice(&second.serialize()).expect("musig pubkey");
    let cache = musig::KeyAggCache::new(&[&first, &second]);
    let internal =
        bitcoin::XOnlyPublicKey::from_slice(&cache.agg_pk().serialize()).expect("xonly key");
    (cache, internal)
}

#[test]
fn sdk_derives_the_canonical_liquid_taproot_vectors() {
    let fixture: Value = serde_json::from_str(GOLDEN).expect("valid golden fixture");
    assert_eq!(value_at(&fixture, "/schemaVersion"), 1);

    let preimage_hash = hex::decode(string_at(&fixture, "/inputs/preimageHash")).unwrap();
    let hashlock = ripemd160::Hash::hash(&preimage_hash);
    let claim = pubkey_from_secret(string_at(&fixture, "/inputs/claimPrivateKey"));
    let refund = pubkey_from_secret(string_at(&fixture, "/inputs/refundPrivateKey"));
    assert_eq!(
        hex::encode(claim.serialize()),
        string_at(&fixture, "/inputs/claimPublicKey")
    );
    assert_eq!(
        hex::encode(refund.serialize()),
        string_at(&fixture, "/inputs/refundPublicKey")
    );
    let timeout = value_at(&fixture, "/inputs/timeoutBlockHeight")
        .as_i64()
        .unwrap();
    let leaf_version =
        LeafVersion::from_u8(value_at(&fixture, "/inputs/leafVersion").as_u64().unwrap() as u8)
            .expect("valid Liquid leaf version");

    for case_name in ["submarine", "reverse"] {
        let case = value_at(&fixture, &format!("/cases/{case_name}"));
        let (claim_script, refund_script) = scripts(case_name, hashlock, claim, refund, timeout);
        assert_eq!(
            hex::encode(claim_script.as_bytes()),
            string_at(case, "/claimLeaf/output")
        );
        assert_eq!(
            hex::encode(refund_script.as_bytes()),
            string_at(case, "/refundLeaf/output")
        );
        assert_eq!(value_at(case, "/claimLeaf/version").as_u64(), Some(196));
        assert_eq!(value_at(case, "/refundLeaf/version").as_u64(), Some(196));

        let (first, second) = if case_name == "submarine" {
            (claim, refund)
        } else {
            (refund, claim)
        };
        let (cache, internal) = musig_internal_key(first, second);
        assert_eq!(
            hex::encode(cache.agg_pk().serialize()),
            string_at(case, "/internalKey")
        );

        let spend_info = TaprootBuilder::new()
            .add_leaf_with_ver(1, claim_script, leaf_version)
            .unwrap()
            .add_leaf_with_ver(1, refund_script, leaf_version)
            .unwrap()
            .finalize(&Secp256k1::new(), internal)
            .expect("complete two-leaf tree");
        let merkle_root = spend_info.merkle_root().expect("two-leaf merkle root");
        assert_eq!(
            hex::encode(merkle_root.to_byte_array()),
            string_at(case, "/merkleRoot")
        );
        assert_eq!(
            hex::encode(spend_info.output_key().into_inner().serialize()),
            string_at(case, "/outputKey")
        );

        for (network, params) in [
            ("mainnet", &AddressParams::LIQUID),
            ("testnet", &AddressParams::LIQUID_TESTNET),
            ("regtest", &AddressParams::ELEMENTS),
        ] {
            let address =
                Address::p2tr(&Secp256k1::new(), internal, Some(merkle_root), None, params);
            assert_eq!(
                address.to_string(),
                string_at(case, &format!("/addresses/{network}"))
            );
            assert!(
                address.blinding_pubkey.is_none(),
                "L-USDT fixture address must be explicit"
            );
        }
    }

    assert_eq!(string_at(&fixture, "/aliases/chainUserLock"), "submarine");
    assert_eq!(string_at(&fixture, "/aliases/chainServerLock"), "reverse");
}
