//! Reconstruction from the recovery wire contract, without a create response.

use std::str::FromStr;

use bitcoin::hashes::{hash160, Hash};
use bitcoin::{absolute::LockTime, script::Instruction, Amount, PublicKey, ScriptBuf};
use elements::secp256k1_zkp::{Keypair, Secp256k1};

use super::bitcoin::BtcSwapScript;
use super::boltz::{Side, SwapRestoreResponse, SwapRestoreType, SwapType};
use super::fees::estimate_claim_fee;
use super::liquid::LiquidSwapScript;
use super::{SwapScript, SwapScriptImpl};
use crate::error::Error;
use crate::network::{Chain, Network};
use crate::util::secrets::Preimage;

impl SwapScript {
    /// Rebuild directly from `swap/restore`, using the key derived at the
    /// selected details' `key_index`. Chain swaps require an explicit side;
    /// submarine/reverse swaps select their refund/claim details automatically.
    ///
    /// The saved tree, keys, timeout, network and lockup address must agree.
    /// The current restore DTO lacks RGB and Liquid token asset context, so
    /// only BTC and L-BTC sides can be reconstructed safely.
    pub fn from_restored(
        network: Network,
        restored: &SwapRestoreResponse,
        our_pubkey: PublicKey,
        side: Option<Side>,
    ) -> Result<Self, Error> {
        let (swap_type, side, claim) = match (&restored.swap_type, side) {
            (SwapRestoreType::Submarine, None) => (SwapType::Submarine, None, false),
            (SwapRestoreType::Reverse, None) => (SwapType::ReverseSubmarine, None, true),
            (SwapRestoreType::Chain, Some(side)) => {
                let claim = side == Side::Claim;
                (SwapType::Chain, Some(side), claim)
            }
            (SwapRestoreType::Chain, None) => {
                return Err(Error::Protocol("Chain restore requires a side".into()));
            }
            _ => {
                return Err(Error::Protocol(
                    "A side is only valid for a chain restore".into(),
                ));
            }
        };
        let missing = || Error::Protocol("Restore is missing the selected side's details".into());
        let (tree, amount, address, server_key, timeout, blinding_key) = if claim {
            let d = restored.claim_details.as_ref().ok_or_else(missing)?;
            (
                &d.tree,
                d.amount,
                &d.lockup_address,
                &d.server_public_key,
                d.timeout_block_height,
                &d.blinding_key,
            )
        } else {
            let d = restored.refund_details.as_ref().ok_or_else(missing)?;
            (
                &d.tree,
                d.amount,
                &d.lockup_address,
                &d.server_public_key,
                d.timeout_block_height,
                &d.blinding_key,
            )
        };
        let asset = if claim { &restored.to } else { &restored.from };
        let chain = match asset.as_str() {
            "BTC" => Chain::Bitcoin(network.into()),
            "L-BTC" => Chain::Liquid(network.into()),
            other => {
                return Err(Error::Protocol(format!(
                    "Cannot restore {other}: the restore response does not provide its asset context"
                )));
            }
        };
        let amount = amount.ok_or_else(|| {
            Error::Protocol("Restore details must include the expected on-chain amount".into())
        })?;
        let server_key = PublicKey::from_str(server_key)?;
        let (sender_pubkey, receiver_pubkey) = if claim {
            (server_key, our_pubkey)
        } else {
            (our_pubkey, server_key)
        };
        let claim_script = ScriptBuf::from_hex(&tree.claim_leaf.output)?;
        let hashlock = claim_script
            .instructions()
            .find_map(|instruction| match instruction {
                Ok(Instruction::PushBytes(bytes)) if bytes.len() == 20 => {
                    Some(hash160::Hash::from_slice(bytes.as_bytes()))
                }
                _ => None,
            })
            .ok_or_else(|| Error::Protocol("Restore tree has no hashlock".into()))??;
        let locktime = LockTime::from_height(timeout)?;
        let expected_hashlock = if claim {
            Some(
                Preimage::from_sha256_str(
                    &restored
                        .claim_details
                        .as_ref()
                        .ok_or_else(missing)?
                        .preimage_hash,
                )?
                .hash160,
            )
        } else {
            restored
                .invoice
                .as_deref()
                .map(Preimage::from_invoice_str)
                .transpose()?
                .map(|preimage| preimage.hash160)
        };
        if expected_hashlock.is_some_and(|expected| expected != hashlock) {
            return Err(Error::Protocol(
                "Restore hashlock does not match payment hash".into(),
            ));
        }
        let script = match chain {
            Chain::Bitcoin(chain) => {
                if blinding_key.is_some() {
                    return Err(Error::Protocol(
                        "Bitcoin restore must not include a blinding key".into(),
                    ));
                }
                let script = BtcSwapScript {
                    swap_type,
                    side,
                    funding_addrs: Some(bitcoin::Address::from_str(address)?.assume_checked()),
                    hashlock,
                    receiver_pubkey,
                    locktime,
                    sender_pubkey,
                    expected_amount: amount,
                    rgb: None,
                };
                script.validate_response_tree(tree)?;
                script.validate_address(chain, address.clone())?;
                SwapScriptImpl::bitcoin(script)
            }
            Chain::Liquid(chain) => {
                let script = LiquidSwapScript {
                    swap_type,
                    side,
                    funding_addrs: Some(elements::Address::from_str(address)?),
                    hashlock,
                    receiver_pubkey,
                    locktime: elements::LockTime::from_height(timeout)?,
                    sender_pubkey,
                    blinding_key: blinding_key
                        .as_deref()
                        .map(|key| Keypair::from_seckey_str(&Secp256k1::new(), key))
                        .transpose()?,
                    asset_context: None,
                    expected_amount: amount,
                };
                script.validate_response_tree(tree)?;
                script.validate_address(chain, address.clone())?;
                SwapScriptImpl::liquid(script)
            }
        };
        // Preserve the same amount pins and magic-routing accounting as create.
        let boltz_lockup = match swap_type {
            SwapType::Submarine => None,
            SwapType::Chain if amount == 0 => None,
            _ => Some(Amount::from_sat(amount)),
        };
        let mrh_amount = match (swap_type, chain) {
            (SwapType::ReverseSubmarine, Chain::Liquid(_)) => {
                Amount::from_sat(amount).checked_sub(estimate_claim_fee(chain, 0.1))
            }
            _ => None,
        };
        Ok(Self::new(script, boltz_lockup, mrh_amount))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::hex::DisplayHex;
    use serde_json::{json, Value};

    use crate::swaps::boltz::{ChainSwapDetails, CreateReverseResponse, CreateSubmarineResponse};

    fn fixture(
        liquid: bool,
        blinded: bool,
        kind: &str,
        side: Option<Side>,
    ) -> (SwapScript, Value, PublicKey) {
        let our_pubkey = PublicKey::from_str(
            "0210154f49ec6656fd70d28abd9bbb71633da124eda324b1bf2b28dd9686915087",
        )
        .unwrap();
        let server_pubkey = PublicKey::from_str(
            "0283d1d7b47cd4163db23e633b81a3a6906a99a99e5acbbe4df29172f42621668c",
        )
        .unwrap();
        let preimage = Preimage::from_str(&"03".repeat(32)).unwrap();
        let claim = kind == "reverse" || side == Some(Side::Claim);
        let swap_type = match kind {
            "submarine" => SwapType::Submarine,
            "reverse" => SwapType::ReverseSubmarine,
            _ => SwapType::Chain,
        };
        let base = BtcSwapScript {
            swap_type,
            side: side.clone(),
            funding_addrs: None,
            hashlock: preimage.hash160,
            sender_pubkey: if claim { server_pubkey } else { our_pubkey },
            receiver_pubkey: if claim { our_pubkey } else { server_pubkey },
            locktime: LockTime::from_height(144).unwrap(),
            expected_amount: 50_000,
            rgb: None,
        };
        let blinding_key = blinded.then(|| "07".repeat(32));
        let address = if liquid {
            LiquidSwapScript {
                swap_type,
                side: side.clone(),
                funding_addrs: None,
                hashlock: base.hashlock,
                sender_pubkey: base.sender_pubkey,
                receiver_pubkey: base.receiver_pubkey,
                locktime: elements::LockTime::from_height(144).unwrap(),
                expected_amount: 50_000,
                blinding_key: blinding_key
                    .as_deref()
                    .map(|key| Keypair::from_seckey_str(&Secp256k1::new(), key).unwrap()),
                asset_context: None,
            }
            .to_address(Network::Regtest.into())
            .unwrap()
            .to_string()
        } else {
            base.to_address(Network::Regtest.into())
                .unwrap()
                .to_string()
        };
        let tree = json!({
            "claimLeaf": {"version": if liquid {196} else {192},
                "output": base.claim_script().as_bytes().to_lower_hex_string()},
            "refundLeaf": {"version": if liquid {196} else {192},
                "output": base.refund_script().as_bytes().to_lower_hex_string()},
        });
        let response = json!({
            "id": "restored-swap", "swapTree": tree, "address": address,
            "lockupAddress": address, "claimPublicKey": server_pubkey.to_string(),
            "refundPublicKey": server_pubkey.to_string(),
            "serverPublicKey": server_pubkey.to_string(), "timeoutBlockHeight": 144,
            "expectedAmount": 50_000, "onchainAmount": 50_000, "amount": 50_000,
            "acceptZeroConf": false, "bip21": "", "blindingKey": blinding_key,
        });
        let chain = if liquid {
            Chain::Liquid(Network::Regtest.into())
        } else {
            Chain::Bitcoin(Network::Regtest.into())
        };
        let original = match kind {
            "submarine" => SwapScript::submarine_from_swap_resp(
                chain,
                &serde_json::from_value::<CreateSubmarineResponse>(response.clone()).unwrap(),
                our_pubkey,
            ),
            "reverse" => SwapScript::reverse_from_swap_resp(
                chain,
                &serde_json::from_value::<CreateReverseResponse>(response.clone()).unwrap(),
                our_pubkey,
            ),
            _ => SwapScript::chain_from_swap_resp(
                chain,
                side.unwrap(),
                serde_json::from_value::<ChainSwapDetails>(response).unwrap(),
                our_pubkey,
            ),
        }
        .unwrap();
        let details = json!({
            "tree": tree, "amount": 50_000, "keyIndex": 7,
            "lockupAddress": address, "serverPublicKey": server_pubkey.to_string(),
            "timeoutBlockHeight": 144, "blindingKey": blinding_key,
            "preimageHash": preimage.sha256.to_string(),
        });
        let asset = if liquid { "L-BTC" } else { "BTC" };
        let restored = json!({
            "id": "restored-swap", "type": kind, "status": "swap.created",
            "createdAt": 1, "from": if claim { "BTC" } else { asset },
            "to": if claim { asset } else { "BTC" },
            "claimDetails": if claim { Some(details.clone()) } else { None },
            "refundDetails": if claim { None } else { Some(details) },
        });
        (original, restored, our_pubkey)
    }

    #[test]
    fn restore_roundtrip_all_kinds_on_both_chains() {
        for (liquid, blinded) in [(false, false), (true, false), (true, true)] {
            for (kind, side) in [
                ("submarine", None),
                ("reverse", None),
                ("chain", Some(Side::Lockup)),
                ("chain", Some(Side::Claim)),
            ] {
                let (original, wire, our_pubkey) = fixture(liquid, blinded, kind, side.clone());
                let restored = SwapScript::from_restored(
                    Network::Regtest,
                    &serde_json::from_value(wire).unwrap(),
                    our_pubkey,
                    side,
                )
                .unwrap();
                match (&original.script, &restored.script) {
                    (SwapScriptImpl::Bitcoin(a), SwapScriptImpl::Bitcoin(b)) => assert_eq!(a, b),
                    (SwapScriptImpl::Liquid(a), SwapScriptImpl::Liquid(b)) => assert_eq!(a, b),
                    _ => panic!("restored the wrong chain"),
                }
                assert_eq!(original.boltz_lockup, restored.boltz_lockup);
                assert_eq!(original.mrh_amount, restored.mrh_amount);
            }
        }
    }

    #[test]
    fn restore_rejects_tampered_or_incomplete_details() {
        for liquid in [false, true] {
            for (kind, side) in [
                ("submarine", None),
                ("reverse", None),
                ("chain", Some(Side::Lockup)),
                ("chain", Some(Side::Claim)),
            ] {
                let (_, wire, key) = fixture(liquid, false, kind, side.clone());
                let field = if kind == "reverse" || side == Some(Side::Claim) {
                    "claimDetails"
                } else {
                    "refundDetails"
                };
                for (path, value) in [
                    ("/amount", Value::Null),
                    ("/serverPublicKey", json!(key.to_string())),
                    ("/timeoutBlockHeight", json!(145)),
                    ("/tree/claimLeaf/version", json!(1)),
                    ("/tree/refundLeaf/output", json!("51")),
                    ("/lockupAddress", json!("invalid")),
                ] {
                    let mut bad = wire.clone();
                    *bad.pointer_mut(&format!("/{field}{path}")).unwrap() = value;
                    assert!(
                        SwapScript::from_restored(
                            Network::Regtest,
                            &serde_json::from_value(bad).unwrap(),
                            key,
                            side.clone(),
                        )
                        .is_err(),
                        "accepted {kind} {field}{path}"
                    );
                }
                assert!(SwapScript::from_restored(
                    Network::Mainnet,
                    &serde_json::from_value(wire).unwrap(),
                    key,
                    side,
                )
                .is_err());
            }
        }
    }
}
