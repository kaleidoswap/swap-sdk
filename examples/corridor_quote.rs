//! Quote the Arkade Intents corridor in both directions, against a live maker.
//!
//! `arkade:BTC <-> lightning:BTC` is not a Boltz-shaped route: the maker serves
//! it as an RFQ over `POST /v1/swap`, beside its `/v2` surface, and answers with
//! a binding quote or a refusal. This example asks for one quote per direction
//! and runs the checks a client owes BEFORE it commits value. Nothing here funds
//! a lockup or pays an invoice.
//!
//! What it deliberately cannot do is finish a swap: funding the send lockup, or
//! claiming the receive lockup once the maker funds it, needs an Ark wallet
//! (`@kaleidorg/swap-sdk/arkade`). Quote, verify, track is the half every
//! runtime carries without that dependency.
//!
//! A quote is a real reservation on the maker's side, so by default the receive
//! request carries an undecodable payout address and the send request a
//! placeholder invoice: the maker refuses both before creating anything. Set
//! `ARK_PAYOUT_ADDRESS` / `BOLT11_INVOICE` + `ARK_REFUND_ADDRESS` to receive
//! real quotes — and then either act on them or let them expire.
//!
//!   MAKER_URL=https://maker.signet.kaleidoswap.com/v2 cargo run --example corridor_quote
use std::time::{SystemTime, UNIX_EPOCH};

use kaleidorg_swap_sdk::bitcoin::hashes::{sha256, Hash};
use kaleidorg_swap_sdk::bitcoin::secp256k1::rand::{self, RngCore};
use kaleidorg_swap_sdk::corridor::{
    new_rfq_id, AmountSide, LightningReceiveRequest, LightningSendRequest, RfqAnswer, RfqQuote,
};
use kaleidorg_swap_sdk::error::Error;
use kaleidorg_swap_sdk::swaps::boltz::BoltzApiClientV2;

/// secp256k1's generator point, x-only: a valid key nobody holds the secret to.
/// Fine for a quote you will not act on; use your own wallet's key otherwise.
const G_X: &str = "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_owned())
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock is after 1970")
        .as_secs()
}

fn describe(quote: &RfqQuote) {
    println!(
        "  quote {}…  give {} sats, receive {} sats (fee {} sats)",
        &quote.rfq_id[..12],
        quote.from_amount,
        quote.to_amount,
        quote.fee_sats()
    );
    println!(
        "  valid until {}, refund deadline {:?}, lockup {:?}",
        quote.valid_until, quote.refund_locktime, quote.profile.lockup_address
    );
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Error> {
    let client = BoltzApiClientV2::new(
        env_or("MAKER_URL", "https://maker.signet.kaleidoswap.com/v2"),
        None,
    );
    println!("corridor: {}", client.corridor_root()?);

    // ---- receive: pay a hold invoice over Lightning, receive on Arkade -------
    // The preimage is YOURS — the maker never sees it until you claim.
    let mut preimage = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut preimage);
    let payment_hash = sha256::Hash::hash(&preimage).to_string();

    println!("\nlightning:BTC->arkade:BTC");
    let receive = client
        .quote_lightning_receive(&LightningReceiveRequest {
            rfq_id: new_rfq_id(),
            // "Receive exactly this on Arkade": the maker inverts it through
            // its rate card, so the price is the free variable and rounds UP
            // by a sat or two — check `to_amount >= amount`, never equality.
            amount_side: AmountSide::To,
            amount: 12_000,
            payment_hash: payment_hash.clone(),
            payout_address: env_or("ARK_PAYOUT_ADDRESS", "not-an-ark-address"),
            payout_pubkey: G_X.to_owned(),
            claim_packet: None,
        })
        .await?;
    match &receive {
        // A refusal is the maker's answer, not an error: priced, deliberate,
        // and returned as a value.
        RfqAnswer::Refusal(refusal) => println!("  refused: {:?}", refusal.reason),
        RfqAnswer::Quote(quote) => {
            describe(quote);
            assert!(quote.to_amount >= 12_000, "the maker rounds up, never down");
            // The invoice is the MAKER's: before it reaches a payer, it must pay
            // OUR hash for exactly `from_amount`, and a payment at the deadline
            // must still leave 30 minutes to claim before the maker's refund
            // opens. Both gates are on the quote.
            let facts = quote.verify_receive_invoice(&payment_hash)?;
            quote.assert_receivable(&facts, now(), Some(quote.from_amount))?;
            println!(
                "  pay {} sats by {} via {}…",
                facts.amount_sats,
                facts.pay_deadline,
                &quote.profile.invoice.as_deref().unwrap_or_default()[..40]
            );
        }
    }

    // ---- send: fund an Arkade lockup, have a Lightning invoice paid ---------
    println!("\narkade:BTC->lightning:BTC");
    let send = client
        .quote_lightning_send(&LightningSendRequest {
            rfq_id: new_rfq_id(),
            invoice: env_or("BOLT11_INVOICE", "lnbc1..."),
            refund_address: env_or("ARK_REFUND_ADDRESS", "not-an-ark-address"),
            client_refund_pubkey: G_X.to_owned(),
        })
        .await?;
    match &send {
        RfqAnswer::Refusal(refusal) => println!("  refused: {:?}", refusal.reason),
        RfqAnswer::Quote(quote) => {
            describe(quote);
            // Immediately before funding — never at quote time: the quote must
            // still be valid and leave 90 minutes before its refund deadline,
            // since that deadline matures against median-time-past. Then derive
            // the covenant from the binding fields with the `./arkade` venue and
            // refuse to fund on any address mismatch.
            quote.assert_fundable(now())?;
            println!(
                "  fund {} sats at {}",
                quote.from_amount,
                quote.profile.lockup_address.as_deref().unwrap_or("?")
            );
        }
    }

    // ---- track: poll by rfq_id until a terminal state -------------------------
    for answer in [&receive, &send] {
        if let RfqAnswer::Quote(quote) = answer {
            match client.rfq_status(&quote.rfq_id).await? {
                Some(status) => println!(
                    "\nstatus {}…: {:?} (terminal: {})",
                    &quote.rfq_id[..12],
                    status.state,
                    status.state.is_terminal()
                ),
                None => println!("\nstatus {}…: unknown id", &quote.rfq_id[..12]),
            }
        }
    }

    // An id the maker never issued is `None`, not an error.
    assert!(client.rfq_status(&"0".repeat(64)).await?.is_none());
    Ok(())
}
