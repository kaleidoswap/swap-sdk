use crate::boltz::Error;
use crate::swap::KeyPair;
use kaleidorg_swap_sdk::network::Network;
use kaleidorg_swap_sdk::util::secrets::SwapMasterKey as CoreSwapMasterKey;
use std::sync::Arc;

#[derive(uniffi::Object)]
pub struct Preimage(pub(crate) kaleidorg_swap_sdk::util::secrets::Preimage);

#[uniffi::export]
impl Preimage {
    #[uniffi::constructor]
    pub fn new() -> Self {
        Self(kaleidorg_swap_sdk::util::secrets::Preimage::random())
    }

    #[uniffi::constructor]
    pub fn from_bytes(vec: Vec<u8>) -> Result<Self, Error> {
        Ok(Self(kaleidorg_swap_sdk::util::secrets::Preimage::from_vec(
            vec,
        )?))
    }

    #[uniffi::method]
    pub fn bytes(&self) -> Option<Vec<u8>> {
        self.0.bytes.map(|b| b.to_vec())
    }

    #[uniffi::method]
    pub fn to_string(&self) -> Option<String> {
        self.0.to_string()
    }

    #[uniffi::method]
    pub fn sha256(&self) -> String {
        self.0.sha256.to_string()
    }

    #[uniffi::method]
    pub fn hash160(&self) -> String {
        self.0.hash160.to_string()
    }
}

impl Default for Preimage {
    fn default() -> Self {
        Self::new()
    }
}

/// BIP85-derived swap keys from a wallet mnemonic.
///
/// One of these per wallet; derive a fresh key and preimage per swap by
/// incrementing `index`. The caller owns the index — nothing here tracks it —
/// and [`Self::master_xpub`] is what the maker's `swap/restore` route needs to
/// hand a reinstalled wallet its outstanding swaps back.
#[derive(uniffi::Object)]
pub struct SwapMasterKey(CoreSwapMasterKey);

#[uniffi::export]
impl SwapMasterKey {
    /// Derive the swap master key from a wallet mnemonic (BIP85 index 26589).
    #[uniffi::constructor]
    pub fn from_wallet_mnemonic(
        wallet_mnemonic: &str,
        passphrase: Option<String>,
        network: Network,
    ) -> Result<Self, Error> {
        Ok(Self(CoreSwapMasterKey::new(
            wallet_mnemonic,
            passphrase.as_deref(),
            network,
        )?))
    }

    /// Reconstruct from the swap (rescue) mnemonic directly.
    #[uniffi::constructor]
    pub fn from_swap_mnemonic(
        mnemonic: &str,
        passphrase: Option<String>,
        network: Network,
    ) -> Result<Self, Error> {
        Ok(Self(CoreSwapMasterKey::from_mnemonic(
            mnemonic,
            passphrase.as_deref(),
            network,
        )?))
    }

    /// The BIP85-derived swap (rescue) mnemonic.
    #[uniffi::method]
    pub fn swap_mnemonic(&self) -> String {
        self.0.mnemonic.to_string()
    }

    /// The master xpub to hand to the swap-restore API.
    #[uniffi::method]
    pub fn master_xpub(&self) -> String {
        self.0.get_master_xpub().to_string()
    }

    /// The swap keypair at `index`.
    ///
    /// The wasm binding returns hex here because JS has no handle for a
    /// keypair; this returns the bound [`KeyPair`] instead, which is what the
    /// swap-script constructors already take.
    #[uniffi::method]
    pub fn derive_swap_key(&self, index: u64) -> Result<Arc<KeyPair>, Error> {
        let keypair = self.0.derive_swapkey(index)?;
        Ok(Arc::new(KeyPair::from_keypair(keypair)))
    }

    /// The deterministic preimage for the swap at `index` (`sha256` of the
    /// derived private key).
    #[uniffi::method]
    pub fn derive_preimage(&self, index: u64) -> Result<Arc<Preimage>, Error> {
        let keypair = self.0.derive_swapkey(index)?;
        Ok(Arc::new(Preimage(
            kaleidorg_swap_sdk::util::secrets::Preimage::from_swap_key(&keypair),
        )))
    }
}
