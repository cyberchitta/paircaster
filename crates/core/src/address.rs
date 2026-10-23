//! The addresses an account has verified it controls.

use crate::profile::{UserDatum, latest};
use crate::proto::UserDataType;
use crate::time::Timestamp;

/// Which chain family an address is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    Ethereum,
    Solana,
}

impl Protocol {
    /// `ethereum` or `solana`, as `--json` writes it.
    pub fn name(self) -> &'static str {
        match self {
            Self::Ethereum => "ethereum",
            Self::Solana => "solana",
        }
    }
}

/// One verified address, as the account's verification message gives it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verification {
    pub address: String,
    pub protocol: Protocol,
    pub chain_id: u32,
    pub timestamp: Timestamp,
}

/// A verified address, and whether it is the account's primary one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Address {
    pub verification: Verification,
    pub primary: bool,
}

/// `verifications` oldest first, each marked primary when the account's
/// user data names it as its primary address for that protocol. Ethereum
/// addresses are compared ignoring case: the primary one is set in mixed
/// case (EIP-55) and verified ones are lowercase.
pub fn assemble(mut verifications: Vec<Verification>, user_data: &[UserDatum]) -> Vec<Address> {
    verifications.sort_by(|a, b| (a.timestamp, &a.address).cmp(&(b.timestamp, &b.address)));
    let eth = latest(user_data, UserDataType::PrimaryAddressEthereum);
    let sol = latest(user_data, UserDataType::PrimaryAddressSolana);
    verifications
        .into_iter()
        .map(|v| {
            let primary = match v.protocol {
                Protocol::Ethereum => eth
                    .as_ref()
                    .is_some_and(|p| p.eq_ignore_ascii_case(&v.address)),
                Protocol::Solana => sol.as_deref() == Some(v.address.as_str()),
            };
            Address {
                verification: v,
                primary,
            }
        })
        .collect()
}
