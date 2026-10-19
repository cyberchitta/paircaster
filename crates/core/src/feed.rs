//! A feed: casts from the accounts one follows, newest first.

use std::cmp::Reverse;
use std::collections::BTreeSet;

use crate::cast::Cast;
use crate::signer::{Signer, app_of};
use crate::time::Timestamp;

/// How far past now a cast's timestamp may be. Snapchain rejects newer
/// messages dated further ahead (v0.14.3, `validations/message.rs`); older
/// ones from before that rule can carry any date.
pub const MAX_FUTURE_SECS: u64 = 600;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedItem {
    pub cast: Cast,
    /// The app whose key signed the cast, per [`app_of`].
    pub app_fid: Option<u64>,
}

/// Which apps' casts a feed keeps, by the app's FID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppFilter {
    Any,
    /// Only casts signed by a key one of these apps asked for. A cast whose
    /// app is unknown is left out.
    Only(BTreeSet<u64>),
    /// Every cast except those from these apps. A cast whose app is unknown
    /// is kept.
    Not(BTreeSet<u64>),
}

impl AppFilter {
    /// Whether a cast from `app_fid` (`None`: app unknown) is kept.
    ///
    /// ```
    /// use selfcast_core::feed::AppFilter;
    /// let only = AppFilter::Only([9152].into());
    /// assert!(only.admits(Some(9152)) && !only.admits(Some(1)) && !only.admits(None));
    /// let not = AppFilter::Not([9152].into());
    /// assert!(!not.admits(Some(9152)) && not.admits(Some(1)) && not.admits(None));
    /// ```
    pub fn admits(&self, app_fid: Option<u64>) -> bool {
        match self {
            AppFilter::Any => true,
            AppFilter::Only(apps) => app_fid.is_some_and(|f| apps.contains(&f)),
            AppFilter::Not(apps) => app_fid.is_none_or(|f| !apps.contains(&f)),
        }
    }
}

/// The newest `limit` of `casts` that `apps` admits, newest first; casts at
/// the same second are ordered by hash, so the order is the same on every
/// run. A cast dated more than [`MAX_FUTURE_SECS`] past `now` is left out:
/// its place in time is unknowable, and it would top the feed for years.
pub fn assemble(
    casts: impl IntoIterator<Item = Cast>,
    signers: &[Signer],
    apps: &AppFilter,
    now: Timestamp,
    limit: usize,
) -> Vec<FeedItem> {
    let latest = Timestamp::from_unix(now.unix_seconds() + MAX_FUTURE_SECS);
    let mut casts: Vec<Cast> = casts
        .into_iter()
        .filter(|c| c.timestamp <= latest)
        .collect();
    casts.sort_by_key(|c| Reverse((c.timestamp, c.id.hash)));
    casts
        .into_iter()
        .map(|cast| FeedItem {
            app_fid: app_of(&cast, signers),
            cast,
        })
        .filter(|item| apps.admits(item.app_fid))
        .take(limit)
        .collect()
}
