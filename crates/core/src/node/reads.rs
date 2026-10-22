//! Reads that span several requests or accounts, for the CLI and the viewer
//! alike. Requests run concurrently without a runtime of their own, so these
//! work under tokio and in a browser.

use std::collections::BTreeSet;
use std::future::Future;

use futures_util::{StreamExt, TryStreamExt, future, stream};

use super::{Node, PageRequest};
use crate::cast::{Cast, CastRef};
use crate::error::Error;
use crate::feed::FeedItem;
use crate::profile::{Names, Users, names_of};
use crate::signer::Signer;
use crate::thread::Thread;
use crate::time::Timestamp;

/// How many accounts are read at once.
const FETCH_AT_ONCE: usize = 8;

/// How many casts a thread reads above the one asked about before it stops.
pub const MAX_ANCESTORS: usize = 50;

impl Node {
    /// The thread around `id`, with the newest `limit` replies, and the user
    /// data of everyone in it.
    pub async fn thread(
        &self,
        id: &CastRef,
        limit: usize,
        now: Timestamp,
    ) -> Result<(Thread, Users), Error> {
        let cast = self.cast_by_id(id.fid, &id.hash).await?;
        let (above, replies) =
            future::try_join(self.ancestors(&cast, MAX_ANCESTORS), self.replies(id)).await?;
        let authors = above
            .casts
            .iter()
            .chain([&cast])
            .chain(&replies)
            .map(|c| c.id.fid)
            .collect();
        let signers = self.signers_of(authors).await?;
        let thread = Thread::assemble(above, cast, replies, &signers, now, limit);
        let users = self.user_data_of(thread.account_fids()).await?;
        Ok((thread, users))
    }

    /// Each account's newest casts and its app keys.
    pub async fn casts_and_signers(
        &self,
        fids: Vec<u64>,
        page: &PageRequest,
    ) -> Result<(Vec<Cast>, Vec<Signer>), Error> {
        let fetched = per_account(fids, |fid| async move {
            let (casts, signers) =
                future::try_join(self.casts_by_fid(fid, page), self.signers_by_fid(fid)).await?;
            Ok((casts.items, signers))
        })
        .await?;
        let (casts, signers): (Vec<_>, Vec<_>) = fetched.into_iter().unzip();
        Ok((casts.concat(), signers.concat()))
    }

    /// The app keys of each of `fids`.
    pub async fn signers_of(&self, fids: BTreeSet<u64>) -> Result<Vec<Signer>, Error> {
        Ok(per_account(fids, |fid| self.signers_by_fid(fid))
            .await?
            .concat())
    }

    /// The user data of each of `fids`.
    pub async fn user_data_of(&self, fids: BTreeSet<u64>) -> Result<Users, Error> {
        let fetched = per_account(fids, |fid| async move {
            Ok((fid, self.user_data_by_fid(fid).await?))
        })
        .await?;
        Ok(fetched.into_iter().collect())
    }

    /// The username of each account mentioned in `items` that has one.
    pub async fn mention_names(&self, items: &[FeedItem]) -> Result<Names, Error> {
        let fids = items
            .iter()
            .flat_map(|i| i.cast.mentions.iter().map(|m| m.fid))
            .collect();
        Ok(names_of(&self.user_data_of(fids).await?))
    }
}

/// `fetch` for each of `fids`, [`FETCH_AT_ONCE`] at a time, in no set order.
/// The first error stops the rest.
async fn per_account<T, F, Fut>(
    fids: impl IntoIterator<Item = u64>,
    fetch: F,
) -> Result<Vec<T>, Error>
where
    F: Fn(u64) -> Fut,
    Fut: Future<Output = Result<T, Error>>,
{
    stream::iter(fids)
        .map(fetch)
        .buffer_unordered(FETCH_AT_ONCE)
        .try_collect()
        .await
}
