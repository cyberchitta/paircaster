//! The custody key: the Ethereum key (secp256k1) that owns an account and
//! can do anything with it, unlike an app key ([`crate::build::AppKey`]),
//! which can only post messages.
//!
//! It is made only from a recovery phrase, is used to authorise an app key,
//! and is then dropped. It is never stored. The phrase, the seed and the key
//! are zeroed on drop.

use std::fmt;

use alloy_primitives::{Address, B256};
use bip32::{DerivationPath, XPrv};
use k256::ecdsa::SigningKey;
use zeroize::Zeroizing;

/// The path of an Ethereum wallet's first account (BIP-44, coin type 60),
/// where wallets and the Farcaster app keep the custody key.
const ETHEREUM_FIRST_ACCOUNT: &str = "m/44'/60'/0'/0/0";

/// An account's custody key. Neither printable nor loggable; zeroed on drop.
pub struct CustodyKey(SigningKey);

/// An Ethereum address, such as a custody key's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EthAddress(Address);

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CustodyError {
    /// Says nothing about the phrase itself, so it cannot leak into a log.
    #[error("not a valid recovery phrase (12 to 24 English BIP-39 words)")]
    BadPhrase,
    #[error("could not derive the custody key from the recovery phrase")]
    Derivation,
}

impl CustodyKey {
    /// The key at Ethereum's first account path, from a BIP-39 recovery
    /// phrase of 12 to 24 English words with no passphrase. Case and spacing
    /// between words do not matter.
    pub fn from_phrase(phrase: &str) -> Result<Self, CustodyError> {
        let words: Vec<&str> = phrase.split_whitespace().collect();
        let mut normalized = Zeroizing::new(words.join(" "));
        normalized.make_ascii_lowercase();
        let mnemonic = Zeroizing::new(
            bip39::Mnemonic::parse_in_normalized(bip39::Language::English, &normalized)
                .map_err(|_| CustodyError::BadPhrase)?,
        );
        let seed = Zeroizing::new(mnemonic.to_seed_normalized(""));
        let path: DerivationPath = ETHEREUM_FIRST_ACCOUNT
            .parse()
            .expect("the Ethereum first-account path parses");
        let xprv =
            XPrv::derive_from_path(&seed[..], &path).map_err(|_| CustodyError::Derivation)?;
        Ok(Self(xprv.private_key().clone()))
    }

    /// The custody address: the last 20 bytes of the Keccak-256 hash of the
    /// uncompressed public key.
    pub fn address(&self) -> EthAddress {
        let point = self.0.verifying_key().to_encoded_point(false);
        EthAddress(Address::from_raw_public_key(&point.as_bytes()[1..]))
    }

    /// Signs a 32-byte EIP-712 hash, as `r || s || v` with `v` 27 or 28, the
    /// form Snapchain recovers (`src/core/validations/key.rs`,
    /// `recover_from_typed_data`).
    pub(crate) fn sign_hash(&self, hash: &B256) -> [u8; 65] {
        let (signature, recovery) = self
            .0
            .sign_prehash_recoverable(hash.as_slice())
            .expect("a 32-byte prehash always signs");
        let mut out = [0u8; 65];
        out[..64].copy_from_slice(&signature.to_bytes());
        out[64] = 27 + recovery.to_byte();
        out
    }
}

impl EthAddress {
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        (bytes.len() == 20).then(|| Self(Address::from_slice(bytes)))
    }

    pub fn as_bytes(&self) -> &[u8; 20] {
        self.0.as_ref()
    }

    pub(crate) fn inner(&self) -> Address {
        self.0
    }
}

/// EIP-55 mixed-case checksum form, e.g. `0x9858EfFD…`.
impl fmt::Display for EthAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0.to_checksum(None))
    }
}
