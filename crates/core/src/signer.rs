//! App keys (signers) and which app asked for each.

use crate::cast::{Cast, SignerKey};

/// One of an account's app keys, as the node lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signer {
    pub fid: u64,
    pub key: SignerKey,
    /// The account of the app that requested the key (`requestFid`), when
    /// the key's registration names one.
    pub app_fid: Option<u64>,
}

/// The app whose key signed `cast`: `None` when the signer is not among
/// `signers` (for instance a key since removed, whose casts remain) or its
/// registration names no app.
pub fn app_of(cast: &Cast, signers: &[Signer]) -> Option<u64> {
    signers
        .iter()
        .find(|s| s.fid == cast.id.fid && s.key == cast.signer)
        .and_then(|s| s.app_fid)
}
