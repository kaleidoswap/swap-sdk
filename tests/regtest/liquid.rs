#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
#[cfg(feature = "electrum")]
use boltz_client::network::electrum::ElectrumConfig;
#[cfg(feature = "esplora")]
use boltz_client::network::esplora::EsploraConfig;
use boltz_client::{
    network::Chain,
    swaps::{
        boltz::{
            BoltzApiClientV2, Cooperative, CreateReverseRequest, CreateSubmarineRequest,
            Subscription, SwapUpdate,
        },
        magic_routing::{check_for_mrh, sign_address},
    },
    util::{secrets::Preimage, setup_logger},
    Bolt11Invoice, LBtcSwapScript, LBtcSwapTx, Secp256k1,
};
use std::str::FromStr;

use crate::regtest::WAIT_TIME_MS;
use crate::utils;
use bitcoin::{
    hashes::{sha256, Hash},
    hex::FromHex,
    key::rand::thread_rng,
    secp256k1::Keypair,
    PublicKey,
};
use boltz_client::boltz::BOLTZ_REGTEST;
use boltz_client::fees::Fee;
use boltz_client::network::esplora::async_sleep;
use boltz_client::network::LiquidClient;
use futures_util::{SinkExt, StreamExt};
use serial_test::serial;
use tokio_tungstenite_wasm::Message;

#[cfg(all(target_family = "wasm", target_os = "unknown"))]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

const CHAIN: Chain = Chain::LiquidRegtest;

#[macros::async_test]
#[serial]
#[cfg(feature = "electrum")]
async fn liquid_v2_submarine_electrum() {
    setup_logger();
    let liquid_client = ElectrumConfig::default(CHAIN, None)
        .build_liquid_client()
        .unwrap();
    liquid_v2_submarine(&liquid_client, false).await;
    liquid_v2_submarine(&liquid_client, true).await;
}

#[macros::async_test_all]
#[serial]
#[cfg(feature = "esplora")]
async fn liquid_v2_submarine_esplora() {
    setup_logger();
    let liquid_client = EsploraConfig::default(CHAIN, None)
        .build_liquid_client()
        .unwrap();
    liquid_v2_submarine(&liquid_client, false).await;
    liquid_v2_submarine(&liquid_client, true).await;
}

async fn liquid_v2_submarine<LC: LiquidClient>(liquid_client: &LC, underpay: bool) {
    let secp = Secp256k1::new();
    let our_keys = Keypair::new(&secp, &mut thread_rng());
    let refund_public_key = PublicKey {
        inner: our_keys.public_key(),
        compressed: true,
    };

    // Set a new invoice string and refund address for each test.
    let invoice = utils::generate_invoice_lnd(50_000).await.unwrap();
    let refund_address = utils::generate_address_elementsd().await.unwrap();
    let boltz_url = BOLTZ_REGTEST;
    let chain = CHAIN;
    let boltz_api_v2 = BoltzApiClientV2::new(boltz_url);

    // If there is MRH send directly to that address
    // if let Some((bip21_addrs, amount)) =
    //     check_for_mrh(&boltz_api_v2, &invoice, CHAIN).unwrap()
    // {
    //     log::info!("Found MRH in invoice");
    //     log::info!("Send {} to {}", amount, bip21_addrs);
    //     return;
    // }
    // Initiate the swap with Boltz
    let create_swap_req = CreateSubmarineRequest {
        from: "L-BTC".to_string(),
        to: "BTC".to_string(),
        invoice: invoice.to_string(),
        refund_public_key,
        pair_hash: None,
        referral_id: None,
        webhook: None,
    };

    let create_swap_response = boltz_api_v2.post_swap_req(&create_swap_req).await.unwrap();
    log::info!("Got Swap Response from Boltz server");

    create_swap_response
        .validate(&invoice, &refund_public_key, chain)
        .unwrap();
    log::info!("VALIDATED RESPONSE!");

    log::debug!("Swap Response: {:?}", create_swap_response);

    let swap_script =
        LBtcSwapScript::submarine_from_swap_resp(&create_swap_response, refund_public_key).unwrap();
    swap_script.to_address(chain).unwrap();

    log::debug!("Created Swap Script. : {:?}", swap_script);

    // Subscribe to websocket updates
    let (mut sender, mut receiver) = boltz_api_v2.connect_ws().await.unwrap().split();

    sender
        .send(Message::text(
            serde_json::to_string(&Subscription::new(&create_swap_response.id)).unwrap(),
        ))
        .await
        .unwrap();

    // Event handlers for various swap status.
    loop {
        let response =
            serde_json::from_str(&receiver.next().await.unwrap().unwrap().into_text().unwrap());

        if response.is_err() {
            if response.expect_err("expected").is_eof() {
                continue;
            }
        } else {
            match response.unwrap() {
                SwapUpdate::Subscription {
                    event,
                    channel,
                    args,
                } => {
                    assert_eq!(event, "subscribe");
                    assert_eq!(channel, "swap.update");
                    assert_eq!(
                        args.first().expect("expected"),
                        &create_swap_response.clone().id
                    );
                    log::info!(
                        "Successfully subscribed for Swap updates. Swap ID : {}",
                        create_swap_response.clone().id
                    );
                }

                SwapUpdate::Update {
                    event,
                    channel,
                    args,
                } => {
                    assert_eq!(event, "update");
                    assert_eq!(channel, "swap.update");
                    let update = args.first().expect("expected");
                    assert_eq!(update.id, create_swap_response.clone().id);
                    log::info!("Got Update from server: {}", update.status);

                    // Invoice is Set. Waiting for us to send onchain tx.
                    if update.status == "invoice.set" {
                        log::info!(
                            "Send {} sats to Liquid address {}",
                            create_swap_response.expected_amount,
                            create_swap_response.address
                        );

                        let amount = match underpay {
                            true => create_swap_response.expected_amount - 1,
                            false => create_swap_response.expected_amount,
                        };
                        utils::send_to_address_elementsd(&create_swap_response.address, amount)
                            .await
                            .unwrap();
                    }

                    if update.status == "transaction.mempool" {
                        utils::mine_blocks(1).await.unwrap();
                    }

                    // Boltz has paid the invoice, and waiting for our partial sig.
                    if update.status == "transaction.claim.pending" {
                        // Create the refund transaction at this stage
                        let swap_tx = LBtcSwapTx::new_refund(
                            swap_script.clone(),
                            &refund_address,
                            liquid_client,
                            boltz_url.to_string(),
                            create_swap_response.clone().id,
                        )
                        .await
                        .unwrap();
                        // why? ^^^s

                        let claim_tx_response = boltz_api_v2
                            .get_submarine_claim_tx_details(&create_swap_response.clone().id)
                            .await
                            .unwrap();

                        log::debug!("Received claim tx details : {:?}", claim_tx_response);

                        // Check that boltz have the correct preimage.
                        // At this stage the client should verify that LN invoice has been paid.
                        let preimage = Vec::from_hex(&claim_tx_response.preimage).unwrap();
                        let preimage_hash = sha256::Hash::hash(&preimage);
                        let invoice = Bolt11Invoice::from_str(&create_swap_req.invoice).unwrap();
                        let invoice_payment_hash = invoice.payment_hash();
                        assert_eq!(invoice_payment_hash.to_string(), preimage_hash.to_string());
                        log::info!("Correct Hash preimage received from Boltz.");

                        // Compute and send Musig2 partial sig
                        let (partial_sig, pub_nonce) = swap_tx
                            .partial_sign(
                                &our_keys,
                                &claim_tx_response.pub_nonce,
                                &claim_tx_response.transaction_hash,
                            )
                            .unwrap();
                        boltz_api_v2
                            .post_submarine_claim_tx_details(
                                &create_swap_response.clone().id,
                                pub_nonce,
                                partial_sig,
                            )
                            .await
                            .unwrap();
                        log::info!("Successfully Sent partial signature");
                    }

                    // This means the funding transaction was rejected by Boltz for whatever reason, and we need to get
                    // fund back via refund.
                    if update.status == "transaction.lockupFailed"
                        || update.status == "invoice.failedToPay"
                    {
                        async_sleep(WAIT_TIME_MS).await;
                        let swap_tx = LBtcSwapTx::new_refund(
                            swap_script.clone(),
                            &refund_address,
                            liquid_client,
                            boltz_url.to_string(),
                            create_swap_response.clone().id,
                        )
                        .await
                        .unwrap();

                        // Coop refund
                        let tx = swap_tx
                            .sign_refund(
                                &our_keys,
                                Fee::Absolute(1000),
                                Some(Cooperative {
                                    boltz_api: &boltz_api_v2,
                                    swap_id: create_swap_response.id.clone(),
                                    pub_nonce: None,
                                    partial_sig: None,
                                }),
                                false,
                            )
                            .await
                            .unwrap();

                        let txid = swap_tx.broadcast(&tx, liquid_client, None).await.unwrap();
                        log::info!("Cooperative Refund Successfully broadcasted: {}", txid);

                        // Non cooperative refund requires expired swap
                        /*log::info!("Attempting Non-cooperative refund.");

                        let tx = swap_tx
                            .sign_refund(&our_keys, Fee::Absolute(1000), None, false)
                            .await
                            .unwrap();
                        let txid = swap_tx
                            .broadcast(&tx, liquid_client, None)
                            .await
                            .unwrap();
                        log::info!("Non-cooperative Refund Successfully broadcasted: {}", txid);
                         */
                        break;
                    }

                    if update.status == "transaction.claimed" {
                        log::info!("Successfully completed submarine swap");
                        break;
                    }
                }

                SwapUpdate::Error {
                    event,
                    channel,
                    args,
                } => {
                    assert_eq!(event, "update");
                    assert_eq!(channel, "swap.update");
                    let error = args.first().expect("expected");
                    log::error!(
                        "Got Boltz response error : {} for swap: {}",
                        error.error,
                        error.id
                    );
                }
            }
        }
    }
}

#[macros::async_test]
#[serial]
#[cfg(feature = "electrum")]
async fn liquid_v2_reverse_electrum() {
    setup_logger();
    let liquid_client = ElectrumConfig::default(CHAIN, None)
        .build_liquid_client()
        .unwrap();
    liquid_v2_reverse(&liquid_client, false).await;
    liquid_v2_reverse(&liquid_client, true).await;
}

#[macros::async_test_all]
#[serial]
#[cfg(feature = "esplora")]
async fn liquid_v2_reverse_esplora() {
    setup_logger();
    let liquid_client = EsploraConfig::default(CHAIN, None)
        .build_liquid_client()
        .unwrap();
    liquid_v2_reverse(&liquid_client, false).await;
    liquid_v2_reverse(&liquid_client, true).await;
}

async fn liquid_v2_reverse<LC: LiquidClient>(liquid_client: &LC, lowball: bool) {
    let secp = Secp256k1::new();
    let preimage = Preimage::new();
    let our_keys = Keypair::new(&secp, &mut thread_rng());
    let invoice_amount = 50_000;
    let claim_public_key = PublicKey {
        compressed: true,
        inner: our_keys.public_key(),
    };

    // Give a valid claim address or else funds will be lost.
    let claim_address = utils::generate_address_elementsd().await.unwrap();
    let boltz_url = BOLTZ_REGTEST;
    let chain = CHAIN;
    let boltz_api_v2 = BoltzApiClientV2::new(boltz_url);

    let addrs_sig = sign_address(&claim_address, &our_keys).unwrap();

    let create_reverse_req = CreateReverseRequest {
        invoice_amount,
        from: "BTC".to_string(),
        to: "L-BTC".to_string(),
        preimage_hash: preimage.sha256,
        description: None,
        description_hash: None,
        address_signature: Some(addrs_sig.to_string()),
        address: Some(claim_address.clone()),
        claim_public_key,
        referral_id: None,
        webhook: None,
    };

    let reverse_resp = boltz_api_v2
        .post_reverse_req(create_reverse_req)
        .await
        .unwrap();
    reverse_resp
        .validate(&preimage, &claim_public_key, chain)
        .unwrap();
    log::info!("VALIDATED RESPONSE!");

    let swap_id = reverse_resp.clone().id;

    let _ = check_for_mrh(&boltz_api_v2, &reverse_resp.invoice, CHAIN)
        .await
        .unwrap()
        .unwrap();

    log::debug!("Got Reverse swap response: {:?}", reverse_resp);

    let swap_script =
        LBtcSwapScript::reverse_from_swap_resp(&reverse_resp, claim_public_key).unwrap();
    swap_script.to_address(CHAIN).unwrap();

    // Subscribe to wss status updates
    let (mut sender, mut receiver) = boltz_api_v2.connect_ws().await.unwrap().split();

    sender
        .send(Message::text(
            serde_json::to_string(&Subscription::new(&swap_id)).unwrap(),
        ))
        .await
        .unwrap();

    // Event handlers for various swap status.
    loop {
        let response =
            serde_json::from_str(&receiver.next().await.unwrap().unwrap().into_text().unwrap());

        if response.is_err() {
            if response.expect_err("expected").is_eof() {
                continue;
            }
        } else {
            match response.as_ref().unwrap() {
                SwapUpdate::Subscription {
                    event,
                    channel,
                    args,
                } => {
                    assert_eq!(event, "subscribe");
                    assert_eq!(channel, "swap.update");
                    assert_eq!(args.first().expect("expected"), &swap_id);
                    log::info!("Subscription successful for swap : {}", &swap_id);
                }

                SwapUpdate::Update {
                    event,
                    channel,
                    args,
                } => {
                    assert_eq!(event, "update");
                    assert_eq!(channel, "swap.update");
                    let update = args.first().expect("expected");
                    assert_eq!(update.id, swap_id);
                    log::info!("Got Update from server: {}", update.status);

                    if update.status == "swap.created" {
                        log::info!("Waiting for Invoice to be paid: {}", &reverse_resp.invoice);

                        let invoice = reverse_resp.invoice.clone();
                        utils::start_pay_invoice_lnd(invoice);

                        continue;
                    }

                    if update.status == "transaction.mempool" {
                        log::info!("Boltz broadcasted funding tx");

                        async_sleep(WAIT_TIME_MS).await;

                        let claim_tx = LBtcSwapTx::new_claim(
                            swap_script.clone(),
                            claim_address.clone(),
                            liquid_client,
                            BOLTZ_REGTEST.to_string(),
                            swap_id.clone(),
                        )
                        .await
                        .unwrap();

                        let tx = claim_tx
                            .sign_claim(
                                &our_keys,
                                &preimage,
                                Fee::Absolute(1000),
                                None,
                                // Some(Cooperative {
                                //     boltz_api: &boltz_api_v2,
                                //     swap_id: swap_id.clone(),
                                //     pub_nonce: None,
                                //     partial_sig: None,
                                // }),
                                false,
                            )
                            .await
                            .unwrap();

                        match lowball {
                            true => {
                                claim_tx
                                    .broadcast(&tx, liquid_client, Some((&boltz_api_v2, CHAIN)))
                                    .await
                                    .unwrap();
                                log::info!("Successfully broadcasted claim tx using lowball!");
                            }
                            false => {
                                claim_tx.broadcast(&tx, liquid_client, None).await.unwrap();
                                log::info!("Successfully broadcasted claim tx!");
                            }
                        }
                        log::debug!("Claim Tx {:?}", tx);
                    }

                    if update.status == "invoice.settled" {
                        log::info!("Reverse Swap Successful!");
                        break;
                    }
                }

                SwapUpdate::Error {
                    event,
                    channel,
                    args,
                } => {
                    assert_eq!(event, "update");
                    assert_eq!(channel, "swap.update");
                    let error = args.first().expect("expected");
                    println!("Got error : {} for swap: {}", error.error, error.id);
                }
            }
        }
    }
}

#[macros::async_test]
#[serial]
#[cfg(feature = "electrum")]
async fn liquid_v2_reverse_script_path_electrum() {
    setup_logger();
    let liquid_client = ElectrumConfig::default(CHAIN, None)
        .build_liquid_client()
        .unwrap();
    liquid_v2_reverse_script_path(&liquid_client, false).await;
    liquid_v2_reverse_script_path(&liquid_client, true).await;
}

#[macros::async_test_all]
#[serial]
#[cfg(feature = "esplora")]
async fn liquid_v2_reverse_script_path_esplora() {
    setup_logger();
    let liquid_client = EsploraConfig::default(CHAIN, None)
        .build_liquid_client()
        .unwrap();
    liquid_v2_reverse_script_path(&liquid_client, false).await;
    liquid_v2_reverse_script_path(&liquid_client, true).await;
}

async fn liquid_v2_reverse_script_path<LC: LiquidClient>(liquid_client: &LC, lowball: bool) {
    let secp = Secp256k1::new();
    let preimage = Preimage::new();
    let our_keys = Keypair::new(&secp, &mut thread_rng());
    let invoice_amount = 50_000;
    let claim_public_key = PublicKey {
        compressed: true,
        inner: our_keys.public_key(),
    };

    // Give a valid claim address or else funds will be lost.
    let claim_address = utils::generate_address_elementsd().await.unwrap();
    let boltz_url = BOLTZ_REGTEST;
    let chain = CHAIN;
    let boltz_api_v2 = BoltzApiClientV2::new(boltz_url);

    let addrs_sig = sign_address(&claim_address, &our_keys).unwrap();

    let create_reverse_req = CreateReverseRequest {
        invoice_amount,
        from: "BTC".to_string(),
        to: "L-BTC".to_string(),
        preimage_hash: preimage.sha256,
        description: None,
        description_hash: None,
        address_signature: Some(addrs_sig.to_string()),
        address: Some(claim_address.clone()),
        claim_public_key,
        referral_id: None,
        webhook: None,
    };

    let reverse_resp = boltz_api_v2
        .post_reverse_req(create_reverse_req)
        .await
        .unwrap();
    reverse_resp
        .validate(&preimage, &claim_public_key, chain)
        .unwrap();
    log::info!("VALIDATED RESPONSE!");

    let swap_id = reverse_resp.clone().id;

    let _ = check_for_mrh(&boltz_api_v2, &reverse_resp.invoice, CHAIN)
        .await
        .unwrap()
        .unwrap();

    log::debug!("Got Reverse swap response: {:?}", reverse_resp);

    let swap_script =
        LBtcSwapScript::reverse_from_swap_resp(&reverse_resp, claim_public_key).unwrap();
    swap_script.to_address(CHAIN).unwrap();

    // Subscribe to wss status updates
    let (mut sender, mut receiver) = boltz_api_v2.connect_ws().await.unwrap().split();

    sender
        .send(Message::text(
            serde_json::to_string(&Subscription::new(&swap_id)).unwrap(),
        ))
        .await
        .unwrap();

    // Event handlers for various swap status.
    loop {
        let response =
            serde_json::from_str(&receiver.next().await.unwrap().unwrap().into_text().unwrap());

        if response.is_err() {
            if response.expect_err("expected").is_eof() {
                continue;
            }
        } else {
            match response.as_ref().unwrap() {
                SwapUpdate::Subscription {
                    event,
                    channel,
                    args,
                } => {
                    assert_eq!(event, "subscribe");
                    assert_eq!(channel, "swap.update");
                    assert_eq!(args.first().expect("expected"), &swap_id);
                    log::info!("Subscription successful for swap : {}", &swap_id);
                }

                SwapUpdate::Update {
                    event,
                    channel,
                    args,
                } => {
                    assert_eq!(event, "update");
                    assert_eq!(channel, "swap.update");
                    let update = args.first().expect("expected");
                    assert_eq!(update.id, swap_id);
                    log::info!("Got Update from server: {}", update.status);

                    if update.status == "swap.created" {
                        log::info!("Waiting for Invoice to be paid: {}", &reverse_resp.invoice);

                        let invoice = reverse_resp.invoice.clone();
                        utils::start_pay_invoice_lnd(invoice);

                        continue;
                    }

                    if update.status == "transaction.mempool" {
                        log::info!("Boltz broadcasted funding tx");

                        async_sleep(WAIT_TIME_MS).await;

                        let claim_tx = LBtcSwapTx::new_claim(
                            swap_script.clone(),
                            claim_address.clone(),
                            liquid_client,
                            BOLTZ_REGTEST.to_string(),
                            swap_id.clone(),
                        )
                        .await
                        .unwrap();

                        let tx = claim_tx
                            .sign_claim(&our_keys, &preimage, Fee::Absolute(1000), None, false)
                            .await
                            .unwrap();

                        match lowball {
                            true => {
                                claim_tx
                                    .broadcast(&tx, liquid_client, Some((&boltz_api_v2, CHAIN)))
                                    .await
                                    .unwrap();
                                log::info!("Successfully broadcasted claim tx using lowball!");
                            }
                            false => {
                                claim_tx.broadcast(&tx, liquid_client, None).await.unwrap();
                                log::info!("Successfully broadcasted claim tx!");
                            }
                        }
                        log::debug!("Claim Tx {:?}", tx);
                    }

                    if update.status == "invoice.settled" {
                        log::info!("Reverse Swap Successful!");
                        break;
                    }
                }

                SwapUpdate::Error {
                    event,
                    channel,
                    args,
                } => {
                    assert_eq!(event, "update");
                    assert_eq!(channel, "swap.update");
                    let error = args.first().expect("expected");
                    println!("Got error : {} for swap: {}", error.error, error.id);
                }
            }
        }
    }
}
