//! Signing a swap spend without holding its key.
//!
//! The claim and refund builders ask a [`SpendSigner`] for what they need:
//! the public key, a BIP340 signature over a Taproot script-path sighash, and
//! for the cooperative key path the two MuSig2 steps. The secret MuSig2 nonce
//! never leaves the signer: a partial signature together with its secret nonce
//! reveals the key, so the signer generates the nonce, keeps it, and spends it
//! on exactly one partial signature.

use std::sync::Mutex;

use bitcoin::secp256k1::{schnorr, Keypair, Message, PublicKey, Secp256k1};
use secp256k1_musig::{musig, Scalar};

use crate::error::Error;
use crate::util::secrets::rng_32b;

/// One MuSig2 key-path signing session, as the signer sees it. Everything the
/// signer needs to rebuild the session is here; only the secret nonce is its
/// own.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MusigSession {
    /// The two keys, in the order the swap script aggregates them.
    pub pubkeys: [PublicKey; 2],
    /// The Taproot tweak applied to the aggregate key.
    pub tweak: [u8; 32],
    /// The key-path sighash being signed.
    pub sighash: [u8; 32],
}

impl MusigSession {
    /// The tweaked key aggregation cache this session signs under.
    pub fn key_agg_cache(&self) -> Result<musig::KeyAggCache, Error> {
        let [a, b] = self.pubkeys.map(to_musig_pubkey);
        let mut cache = musig::KeyAggCache::new(&[&a, &b]);
        let tweak = Scalar::from_be_bytes(self.tweak)?;
        cache.pubkey_xonly_tweak_add(&tweak)?;
        Ok(cache)
    }

    fn ensure_signs_for(&self, pubkey: &PublicKey) -> Result<(), Error> {
        if !self.pubkeys.contains(pubkey) {
            return Err(Error::Protocol(
                "MuSig2 session does not include the signer's key".to_string(),
            ));
        }
        Ok(())
    }
}

/// Signs for one swap key. [`KeypairSigner`] holds the key in memory; other
/// implementations can keep it in a hardware wallet, an enclave or a remote
/// service.
#[macros::async_trait]
pub trait SpendSigner: Send + Sync {
    fn public_key(&self) -> PublicKey;
    /// A BIP340 signature with the untweaked key over a Taproot script-path
    /// sighash.
    async fn sign_schnorr(&self, sighash: [u8; 32]) -> Result<schnorr::Signature, Error>;
    /// Generates and keeps a fresh secret nonce for `session` and returns its
    /// public nonce.
    async fn musig_nonce(&self, session: &MusigSession) -> Result<musig::PublicNonce, Error>;
    /// Signs `session` with the secret nonce behind `our_nonce` and forgets
    /// that nonce. A nonce this signer did not hand out, or already used, is
    /// an error.
    async fn musig_partial_sign(
        &self,
        session: &MusigSession,
        our_nonce: &musig::PublicNonce,
        their_nonce: &musig::PublicNonce,
    ) -> Result<musig::PartialSignature, Error>;
}

/// Secret MuSig2 nonces handed out and not yet used. A signer that holds its
/// key in memory keeps one of these next to it.
#[derive(Default)]
pub struct MusigNonces {
    pending: Mutex<Vec<(musig::PublicNonce, musig::SecretNonce)>>,
}

/// Sessions abandoned before their partial signature (the counterparty never
/// answered) leave a nonce behind; past this many the oldest is dropped.
const MAX_PENDING_NONCES: usize = 64;

impl MusigNonces {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn generate(
        &self,
        keys: &Keypair,
        session: &MusigSession,
    ) -> Result<musig::PublicNonce, Error> {
        let pubkey = keys.public_key();
        session.ensure_signs_for(&pubkey)?;
        let cache = session.key_agg_cache()?;
        let rand = musig::SessionSecretRand::assume_unique_per_nonce_gen(rng_32b());
        let (sec_nonce, pub_nonce) = cache.nonce_gen(
            rand,
            to_musig_pubkey(pubkey),
            &session.sighash,
            Some(rng_32b()),
        );
        let mut pending = self.lock()?;
        if pending.len() >= MAX_PENDING_NONCES {
            pending.remove(0);
        }
        pending.push((pub_nonce, sec_nonce));
        Ok(pub_nonce)
    }

    pub fn sign(
        &self,
        keys: &Keypair,
        session: &MusigSession,
        our_nonce: &musig::PublicNonce,
        their_nonce: &musig::PublicNonce,
    ) -> Result<musig::PartialSignature, Error> {
        session.ensure_signs_for(&keys.public_key())?;
        let cache = session.key_agg_cache()?;
        let sec_nonce = {
            let mut pending = self.lock()?;
            let at = pending
                .iter()
                .position(|(pub_nonce, _)| pub_nonce == our_nonce)
                .ok_or_else(|| {
                    Error::Protocol("MuSig2 nonce is unknown or already used".to_string())
                })?;
            pending.remove(at).1
        };
        let agg_nonce = musig::AggregatedNonce::new(&[their_nonce, our_nonce]);
        let musig_session = musig::Session::new(&cache, agg_nonce, &session.sighash);
        Ok(musig_session.partial_sign(sec_nonce, &convert_keypair(keys), &cache))
    }

    fn lock(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, Vec<(musig::PublicNonce, musig::SecretNonce)>>, Error>
    {
        self.pending
            .lock()
            .map_err(|_| Error::Generic("MuSig2 nonce store poisoned".to_string()))
    }
}

/// A [`SpendSigner`] over a key held in memory.
pub struct KeypairSigner {
    keys: Keypair,
    nonces: MusigNonces,
}

impl KeypairSigner {
    pub fn new(keys: Keypair) -> Self {
        Self {
            keys,
            nonces: MusigNonces::new(),
        }
    }
}

#[macros::async_trait]
impl SpendSigner for KeypairSigner {
    fn public_key(&self) -> PublicKey {
        self.keys.public_key()
    }

    async fn sign_schnorr(&self, sighash: [u8; 32]) -> Result<schnorr::Signature, Error> {
        Ok(sign_schnorr(&self.keys, sighash))
    }

    async fn musig_nonce(&self, session: &MusigSession) -> Result<musig::PublicNonce, Error> {
        self.nonces.generate(&self.keys, session)
    }

    async fn musig_partial_sign(
        &self,
        session: &MusigSession,
        our_nonce: &musig::PublicNonce,
        their_nonce: &musig::PublicNonce,
    ) -> Result<musig::PartialSignature, Error> {
        self.nonces
            .sign(&self.keys, session, our_nonce, their_nonce)
    }
}

/// BIP340 with an in-memory key, the operation [`SpendSigner::sign_schnorr`]
/// stands for.
pub fn sign_schnorr(keys: &Keypair, sighash: [u8; 32]) -> schnorr::Signature {
    Secp256k1::new().sign_schnorr(&Message::from_digest(sighash), keys)
}

/// Asks `signer` for a script-path signature over `sighash` and checks it
/// against the signer's key before it goes into a witness.
pub(crate) async fn script_path_signature(
    signer: &dyn SpendSigner,
    sighash: [u8; 32],
) -> Result<schnorr::Signature, Error> {
    let signature = signer.sign_schnorr(sighash).await?;
    let (xonly, _) = signer.public_key().x_only_public_key();
    Secp256k1::verification_only()
        .verify_schnorr(&signature, &Message::from_digest(sighash), &xonly)
        .map_err(|_| Error::Protocol("Signer returned an invalid signature".to_string()))?;
    Ok(signature)
}

/// Finishes a cooperative key-path spend once the counterparty answered our
/// nonce: checks its partial signature, asks `signer` for ours, checks that
/// too, and aggregates.
pub(crate) async fn finish_musig(
    signer: &dyn SpendSigner,
    session: &MusigSession,
    our_nonce: &musig::PublicNonce,
    their_nonce: &musig::PublicNonce,
    their_partial: &musig::PartialSignature,
    their_pubkey: PublicKey,
) -> Result<secp256k1_musig::schnorr::Signature, Error> {
    let cache = session.key_agg_cache()?;
    let agg_nonce = musig::AggregatedNonce::new(&[their_nonce, our_nonce]);
    let musig_session = musig::Session::new(&cache, agg_nonce, &session.sighash);
    if !musig_session.partial_verify(
        &cache,
        their_partial,
        their_nonce,
        to_musig_pubkey(their_pubkey),
    ) {
        return Err(Error::Protocol(
            "Invalid partial-sig received from Boltz".to_string(),
        ));
    }
    let ours = signer
        .musig_partial_sign(session, our_nonce, their_nonce)
        .await?;
    if !musig_session.partial_verify(
        &cache,
        &ours,
        our_nonce,
        to_musig_pubkey(signer.public_key()),
    ) {
        return Err(Error::Protocol(
            "Signer returned an invalid partial signature".to_string(),
        ));
    }
    Ok(musig_session
        .partial_sig_agg(&[their_partial, &ours])
        .assume_valid())
}

use super::bitcoin::{convert_keypair, convert_public_key as to_musig_pubkey};

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::key::rand::rngs::OsRng;

    fn session(ours: &Keypair, theirs: &Keypair) -> MusigSession {
        MusigSession {
            pubkeys: [theirs.public_key(), ours.public_key()],
            tweak: [7; 32],
            sighash: [9; 32],
        }
    }

    #[macros::async_test_all]
    async fn a_nonce_signs_once() {
        let secp = Secp256k1::new();
        let ours = Keypair::new(&secp, &mut OsRng);
        let theirs = Keypair::new(&secp, &mut OsRng);
        let session = session(&ours, &theirs);
        let signer = KeypairSigner::new(ours);
        let their_signer = KeypairSigner::new(theirs);

        let our_nonce = signer.musig_nonce(&session).await.unwrap();
        let their_nonce = their_signer.musig_nonce(&session).await.unwrap();
        let their_partial = their_signer
            .musig_partial_sign(&session, &their_nonce, &our_nonce)
            .await
            .unwrap();
        let signature = finish_musig(
            &signer,
            &session,
            &our_nonce,
            &their_nonce,
            &their_partial,
            theirs.public_key(),
        )
        .await
        .unwrap();

        let agg = session.key_agg_cache().unwrap().agg_pk();
        secp256k1_musig::schnorr::verify(&signature, &session.sighash, &agg).unwrap();

        assert!(signer
            .musig_partial_sign(&session, &our_nonce, &their_nonce)
            .await
            .is_err());
    }

    #[macros::async_test_all]
    async fn a_session_without_the_signers_key_is_refused() {
        let secp = Secp256k1::new();
        let ours = Keypair::new(&secp, &mut OsRng);
        let a = Keypair::new(&secp, &mut OsRng);
        let b = Keypair::new(&secp, &mut OsRng);
        let signer = KeypairSigner::new(ours);
        assert!(signer.musig_nonce(&session(&a, &b)).await.is_err());
    }
}
