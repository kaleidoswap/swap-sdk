//! Compatibility with maker PR #551 and rgb-lib, independent of SDK-generated fixtures.
use std::str::FromStr;

use bitcoin::{Address, OutPoint, Psbt, PublicKey, ScriptBuf};
use kaleidorg_swap_sdk::error::Error;
use kaleidorg_swap_sdk::network::BitcoinChain;
use kaleidorg_swap_sdk::swaps::boltz::{
    CreateReverseResponse, CreateSubmarineResponse, GetReversePairsResponse,
    GetSubmarinePairsResponse, SwapTxKind,
};
use kaleidorg_swap_sdk::swaps::rgb::{
    rgb_recipient_id, rgb_recipient_script, PreparedRgbSpend, RgbChainNet, RgbLock, RgbSpendFunding,
};
use kaleidorg_swap_sdk::util::secrets::Preimage;
use kaleidorg_swap_sdk::BtcSwapScript;
use serde_json::{json, Value};

const WIRE: &str = include_str!("fixtures/rgb-v1/wire-contract.json");
const GOLDEN: &str = include_str!("fixtures/rgb-v1/rgb-golden-vectors.json");
const CHAIN: BitcoinChain = BitcoinChain::BitcoinRegtest;

fn wire() -> Value {
    serde_json::from_str(WIRE).unwrap()
}
fn text(value: &Value) -> &str {
    value.as_str().expect("fixture string")
}
fn validate(fixture: &Value, direction: &str, response: Value) -> Result<(), Error> {
    let request = &fixture["create"][direction]["request"];
    let contract = text(&fixture["contractId"]);
    if direction == "submarine" {
        let response: CreateSubmarineResponse = serde_json::from_value(response).unwrap();
        response.validate_rgb(
            text(&request["invoice"]),
            &PublicKey::from_str(text(&request["refundPublicKey"])).unwrap(),
            CHAIN,
            contract,
        )
    } else {
        let response: CreateReverseResponse = serde_json::from_value(response).unwrap();
        response.validate_rgb(
            &Preimage::from_str(text(&fixture["preimage"])).unwrap(),
            &PublicKey::from_str(text(&request["claimPublicKey"])).unwrap(),
            CHAIN,
            contract,
        )
    }
}

#[test]
fn maker_create_responses_bind_keys_hashlock_address_and_rgb_lock() {
    let f = wire();
    assert_eq!(f["schemaVersion"], 1);
    assert_eq!(f["provenance"]["assetIssued"], false);
    for direction in ["submarine", "reverse"] {
        let response = f["create"][direction]["response"].clone();
        validate(&f, direction, response.clone()).unwrap();
        let lock: RgbLock = serde_json::from_value(response["rgb"].clone()).unwrap();
        assert_eq!(serde_json::to_value(lock).unwrap(), response["rgb"]);
        assert!(!response.get("assetId").is_some_and(Value::is_string));
        assert!(!response.get("feeAssetId").is_some_and(Value::is_string));
        assert!(!response["rgb"]["blinding"].is_number());
    }
    assert!(f["create"]["submarine"]["response"]["rgb"]
        .get("claimFeeRate")
        .is_none());
    assert_eq!(f["create"]["reverse"]["response"]["rgb"]["claimFeeRate"], 5);
}

#[test]
fn pair_cards_keep_the_units_of_each_asset_leg() {
    let f = wire();
    assert_eq!(f["amountSemantics"]["limits"], "inputAssetBaseUnits");
    assert_eq!(f["amountSemantics"]["quotedFees"], "outputAssetBaseUnits");
    let submarine: GetSubmarinePairsResponse =
        serde_json::from_value(f["pairResponses"]["submarine"].clone()).unwrap();
    let reverse: GetReversePairsResponse =
        serde_json::from_value(f["pairResponses"]["reverse"].clone()).unwrap();
    assert!(submarine.get_usdt_rgb_to_btc_pair().is_some());
    assert!(reverse.get_btc_to_usdt_rgb_pair().is_some());
    let sub = &f["pairResponses"]["submarine"]["USDT-RGB"]["BTC"];
    let rev = &f["pairResponses"]["reverse"]["BTC"]["USDT-RGB"];
    assert_eq!(sub["rate"], 100.0); // sats per 6-decimal contract unit
    assert_eq!(rev["rate"], 0.01); // contract units per sat
    assert_eq!(sub["limits"]["minimal"], 10); // input RGB units
    assert_eq!(rev["limits"]["minimal"], 1_000); // input BTC sats
    assert_eq!(rev["fees"]["minerFees"]["lockup"], 0);
    assert_eq!(rev["fees"]["minerFees"]["claim"], 31); // rounded up RGB units
    assert_eq!(sub["fees"]["minerFees"], 470); // output BTC sats
    let invoice_sat = 100_000u64;
    assert_eq!(
        f["create"]["submarine"]["response"]["expectedAmount"],
        (invoice_sat + 470).div_ceil(100)
    );
    // Reverse lock pays 1521 sats plus 312 vB * 5 sat/vB; payout rounds down.
    assert_eq!(
        f["create"]["reverse"]["response"]["onchainAmount"],
        (invoice_sat - 1_521 - 1_560) / 100
    );
    assert_eq!(f["create"]["submarine"]["response"]["rgb"]["amount"], 1_005);
    assert_eq!(f["create"]["reverse"]["response"]["rgb"]["amount"], 969);
}

#[test]
fn recipient_codec_matches_the_pinned_rgb_library_on_real_htlc_keys() {
    let golden: Value = serde_json::from_str(GOLDEN).unwrap();
    let vectors = golden["recipients"].as_array().unwrap();
    assert_eq!(vectors.len(), 12);
    let networks = vectors
        .iter()
        .map(|v| text(&v["network"]))
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        networks,
        ["bc", "tb3", "tb4", "sb", "sbc", "bcrt"]
            .into_iter()
            .collect()
    );
    for vector in vectors {
        let script = ScriptBuf::from_hex(text(&vector["scriptPubkey"])).unwrap();
        let network = RgbChainNet::from_prefix(text(&vector["network"])).unwrap();
        let recipient = text(&vector["recipientId"]);
        assert_eq!(rgb_recipient_id(&script, network).unwrap(), recipient);
        assert_eq!(rgb_recipient_script(recipient).unwrap(), (network, script));
        let mut corrupt = recipient.to_owned();
        corrupt.pop();
        corrupt.push('!');
        assert!(rgb_recipient_script(&corrupt).is_err());
    }
}

#[test]
fn sdk_claim_matches_the_maker_reference_psbt_and_fee_budget() {
    let f = wire();
    let golden: Value = serde_json::from_str(GOLDEN).unwrap();
    let reference = &golden["takerClaim"];
    let maker = Psbt::from_str(text(&reference["psbt"])).unwrap();
    let response: CreateReverseResponse =
        serde_json::from_value(f["create"]["reverse"]["response"].clone()).unwrap();
    let key =
        PublicKey::from_str(text(&f["create"]["reverse"]["request"]["claimPublicKey"])).unwrap();
    let script = BtcSwapScript::reverse_from_swap_resp(&response, key).unwrap();
    assert_eq!(script.expected_amount, 1_521); // BTC sats, never RGB contract units
    assert_eq!(script.rgb.as_ref().unwrap().amount, 969);
    let payout_script = ScriptBuf::from_hex(text(&reference["destinationScript"])).unwrap();
    let address = Address::from_script(&payout_script, bitcoin::Network::Regtest).unwrap();
    let prepare = |rate, max_fee| {
        PreparedRgbSpend::new(
            SwapTxKind::Claim,
            script.clone(),
            &address.to_string(),
            CHAIN,
            (
                OutPoint::from_str(text(&reference["outpoint"])).unwrap(),
                maker.inputs[0].witness_utxo.clone().unwrap(),
            ),
            RgbSpendFunding::HtlcValue {
                fee_rate_sat_vb: rate,
            },
            max_fee,
        )
    };
    let template = prepare(5, 975).unwrap().template();
    let sdk = Psbt::from_str(&template.psbt).unwrap();
    assert_eq!(sdk.unsigned_tx, maker.unsigned_tx);
    assert_eq!(sdk.inputs[0].witness_utxo, maker.inputs[0].witness_utxo);
    assert_eq!(template.payment_value, 546);
    assert_eq!(template.amount, 969);
    assert!(!template.requires_funding);
    assert!(prepare(5, 974).is_err());
    assert!(matches!(prepare(6, 2_000), Err(Error::RgbFeeInputRequired)));
}

#[test]
fn changing_any_frozen_response_binding_is_refused() {
    let f = wire();
    let edits = [
        ("/rgb", Value::Null),
        ("/rgb/assetId", json!("rgb:another-contract")),
        ("/rgb/amount", json!(1)),
        ("/rgb/recipientId", json!("bcrt:wvout:corrupt")),
        (
            "/rgb/scriptPubkey",
            json!("51200101010101010101010101010101010101010101010101010101010101010101"),
        ),
        ("/rgb/blinding", json!("18446744073709551616")),
        ("/rgb/htlcSat", json!(545)),
        ("/rgb/minConfirmations", json!(0)),
        (
            "/rgb/transportEndpoints",
            json!(["https://proxy.test/json-rpc"]),
        ),
        ("/rgb/transportEndpoints", json!([])),
        ("/timeoutBlockHeight", json!(500)),
    ];
    for direction in ["submarine", "reverse"] {
        for (pointer, replacement) in &edits {
            let mut response = f["create"][direction]["response"].clone();
            *response.pointer_mut(pointer).unwrap() = replacement.clone();
            assert!(
                validate(&f, direction, response).is_err(),
                "accepted {direction} {pointer}"
            );
        }
        let mut response = f["create"][direction]["response"].clone();
        response["rgb"]["recipientId"] =
            json!(text(&response["rgb"]["recipientId"]).replacen("bcrt:", "bc:", 1));
        assert!(validate(&f, direction, response).is_err());
        let mut response = f["create"][direction]["response"].clone();
        let other = &f["create"][if direction == "reverse" {
            "submarine"
        } else {
            "reverse"
        }]["response"];
        let field = if direction == "reverse" {
            "lockupAddress"
        } else {
            "address"
        };
        response[field] = other[if direction == "reverse" {
            "address"
        } else {
            "lockupAddress"
        }]
        .clone();
        assert!(validate(&f, direction, response).is_err());
    }
    for rate in [Value::Null, json!(0), json!(6)] {
        let mut response = f["create"]["reverse"]["response"].clone();
        response["rgb"]["claimFeeRate"] = rate;
        assert!(validate(&f, "reverse", response).is_err());
    }
}

#[test]
fn rgb_responses_require_canonical_leaf_versions_and_script_bytes() {
    let f = wire();
    for direction in ["submarine", "reverse"] {
        for leaf in ["claimLeaf", "refundLeaf"] {
            let mut response = f["create"][direction]["response"].clone();
            response["swapTree"][leaf]["version"] = json!(194);
            assert!(
                validate(&f, direction, response).is_err(),
                "accepted {direction} {leaf} version"
            );
            let mut response = f["create"][direction]["response"].clone();
            response["swapTree"][leaf]["output"] =
                json!(format!("{}00", text(&response["swapTree"][leaf]["output"])));
            assert!(
                validate(&f, direction, response).is_err(),
                "accepted {direction} {leaf} extra opcode"
            );
            let mut response = f["create"][direction]["response"].clone();
            let key_field = if direction == "submarine" {
                "refundPublicKey"
            } else {
                "claimPublicKey"
            };
            let key = &text(&f["create"][direction]["request"][key_field])[2..];
            response["swapTree"][leaf]["output"] =
                json!(text(&response["swapTree"][leaf]["output"]).replace(key, &"01".repeat(32)));
            // Only the leaf containing our key was changed.
            if response != f["create"][direction]["response"] {
                assert!(
                    validate(&f, direction, response).is_err(),
                    "accepted {direction} {leaf} wrong key"
                );
            }
        }
        let mut response = f["create"][direction]["response"].clone();
        let output = text(&response["swapTree"]["claimLeaf"]["output"]).to_owned();
        // Preserve the shape but replace the hash160 pushed after OP_HASH160.
        let hash_offset = output.find("a914").unwrap() + 4;
        response["swapTree"]["claimLeaf"]["output"] = json!(format!(
            "{}{}{}",
            &output[..hash_offset],
            "00".repeat(20),
            &output[hash_offset + 40..]
        ));
        assert!(
            validate(&f, direction, response).is_err(),
            "accepted {direction} wrong hashlock"
        );
    }
}

struct ReviewCoins(Vec<(OutPoint, bitcoin::TxOut)>);
#[macros::async_trait]
impl kaleidorg_swap_sdk::network::BitcoinClient for ReviewCoins {
    async fn get_address_balance(&self, _: &Address) -> Result<(u64, i64), Error> {
        unreachable!()
    }
    async fn get_address_utxos(
        &self,
        _: &Address,
    ) -> Result<Vec<(OutPoint, bitcoin::TxOut)>, Error> {
        Ok(self.0.clone())
    }
    async fn get_tx(&self, _: bitcoin::Txid) -> Result<bitcoin::Transaction, Error> {
        unreachable!()
    }
    async fn broadcast_tx(&self, _: &bitcoin::Transaction) -> Result<bitcoin::Txid, Error> {
        unreachable!()
    }
    fn network(&self) -> BitcoinChain {
        CHAIN
    }
}
#[tokio::test]
async fn rgb_spends_ignore_address_candidates_and_require_the_pinned_lock() {
    use kaleidorg_swap_sdk::swaps::boltz::BoltzApiClientV2;
    use kaleidorg_swap_sdk::swaps::{ChainClient, RgbPsbtParams, SwapScript};
    let f = wire();
    let g: Value = serde_json::from_str(GOLDEN).unwrap();
    let dest = Address::from_script(
        &ScriptBuf::from_hex(text(&g["takerClaim"]["destinationScript"])).unwrap(),
        bitcoin::Network::Regtest,
    )
    .unwrap()
    .to_string();
    let api = BoltzApiClientV2::new("http://127.0.0.1:1/v2".into(), None);
    for direction in ["submarine", "reverse"] {
        let req = &f["create"][direction]["request"];
        let script = if direction == "submarine" {
            BtcSwapScript::submarine_from_swap_resp(
                &serde_json::from_value::<CreateSubmarineResponse>(
                    f["create"][direction]["response"].clone(),
                )
                .unwrap(),
                PublicKey::from_str(text(&req["refundPublicKey"])).unwrap(),
            )
            .unwrap()
        } else {
            BtcSwapScript::reverse_from_swap_resp(
                &serde_json::from_value::<CreateReverseResponse>(
                    f["create"][direction]["response"].clone(),
                )
                .unwrap(),
                PublicKey::from_str(text(&req["claimPublicKey"])).unwrap(),
            )
            .unwrap()
        };
        let c = script.rgb.as_ref().unwrap();
        let real = bitcoin::Transaction {
            version: bitcoin::transaction::Version::TWO,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: vec![],
            output: vec![bitcoin::TxOut {
                value: bitcoin::Amount::from_sat(c.htlc_sat),
                script_pubkey: c.script_pubkey.clone(),
            }],
        };
        let rp = OutPoint::new(real.compute_txid(), 0);
        let fake = OutPoint::new(bitcoin::Txid::from_str(&"11".repeat(32)).unwrap(), 0);
        let fo = bitcoin::TxOut {
            value: bitcoin::Amount::from_sat(if direction == "submarine" {
                546
            } else {
                c.htlc_sat
            }),
            script_pubkey: c.script_pubkey.clone(),
        };
        let chain = ChainClient::new()
            .with_bitcoin(ReviewCoins(vec![(fake, fo), (rp, real.output[0].clone())]));
        let swap = SwapScript::from_bitcoin(script);
        for pin in [
            real.clone(),
            bitcoin::Transaction {
                output: vec![],
                ..real.clone()
            },
        ] {
            let has_htlc = !pin.output.is_empty();
            let params = RgbPsbtParams {
                output_address: dest.clone(),
                funding: RgbSpendFunding::CallerInputs,
                max_fee: 10_000,
                swap_id: "review-probe".into(),
                chain_client: &chain,
                boltz_api: &api,
                lockup_tx: pin,
            };
            let prepared = if direction == "submarine" {
                swap.prepare_rgb_refund(params).await
            } else {
                swap.prepare_rgb_claim(params).await
            };
            if has_htlc {
                assert_eq!(prepared.unwrap().template().swap_outpoint, rp.to_string());
            } else {
                assert!(prepared
                    .unwrap_err()
                    .message()
                    .contains("supplied lock transaction"));
            }
        }
    }
}

#[test]
fn submarine_collateral_is_capped_before_locking() {
    let f = wire();
    let request = &f["create"]["submarine"]["request"];
    let key = PublicKey::from_str(text(&request["refundPublicKey"])).unwrap();
    for amount in [1_001, 5_000_000, u64::MAX] {
        let mut value = f["create"]["submarine"]["response"].clone();
        value["rgb"]["htlcSat"] = json!(amount);
        let response: CreateSubmarineResponse = serde_json::from_value(value.clone()).unwrap();
        assert!(validate(&f, "submarine", value)
            .unwrap_err()
            .message()
            .contains("collateral cap"));
        response
            .validate_rgb_with_max_htlc_sat(
                text(&request["invoice"]),
                &key,
                CHAIN,
                text(&f["contractId"]),
                amount,
            )
            .unwrap();
        assert!(response
            .validate_rgb_with_max_htlc_sat(
                text(&request["invoice"]),
                &key,
                CHAIN,
                text(&f["contractId"]),
                amount - 1
            )
            .is_err());
    }
    validate(
        &f,
        "submarine",
        f["create"]["submarine"]["response"].clone(),
    )
    .unwrap();
}

#[test]
fn contract_chunk_dashes_do_not_change_the_pinned_asset() {
    let f = wire();
    let mut response = f["create"]["submarine"]["response"].clone();
    response["rgb"]["assetId"] = json!(text(&response["rgb"]["assetId"]).replace('-', ""));
    validate(&f, "submarine", response).unwrap();
}
