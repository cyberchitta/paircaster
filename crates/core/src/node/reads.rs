//! Reads that span several requests or accounts, for the CLI and the viewer
//! alike. Requests run concurrently without a runtime of their own, so these
//! work under tokio and in a browser.

use std::collections::{BTreeSet, VecDeque};
use std::future::Future;

use futures_util::{StreamExt, TryStreamExt, future, stream};

use super::{Node, PageRequest};
use crate::account::Account;
use crate::address::{self, Address};
use crate::cast::{Cast, CastRef, ChannelUrl};
use crate::error::Error;
use crate::event::{Event, EventWalk};
use crate::feed::{self, AppFilter, FeedItem};
use crate::follow::Follow;
use crate::profile::{Profile, Users};
use crate::proto::ReactionType;
use crate::reaction::CastReactions;
use crate::signer::Signer;
use crate::thread::Thread;
use crate::time::Timestamp;

/// How many accounts are read at once.
const FETCH_AT_ONCE: usize = 8;

/// How many casts a thread reads above the one asked about before it stops.
pub const MAX_ANCESTORS: usize = 50;

/// How many events one request asks for.
const EVENT_PAGE_SIZE: u32 = 10_000;

/// A read of the events in a window, shard after shard, a page at a time,
/// from [`Node::event_read`] and advanced by [`Node::next_events`].
#[derive(Debug)]
pub struct EventRead {
    shards: VecDeque<(u32, u64)>,
    since: Option<Timestamp>,
    until: Option<Timestamp>,
    walk: Option<EventWalk>,
}

impl Node {
    /// `fid`'s profile, its followers counted up to `cap`, or all of them
    /// when `None`. An account the node has no registration for is an
    /// error.
    pub async fn profile(&self, fid: u64, cap: Option<usize>) -> Result<Profile, Error> {
        if !self.is_registered(fid).await? {
            return Err(Error::NotRegistered(fid));
        }
        let (user_data, following, followers) = future::try_join3(
            self.user_data_by_fid(fid),
            self.all_follows_by_fid(fid),
            self.follower_count(fid, cap),
        )
        .await?;
        Ok(Profile::assemble(
            fid,
            &user_data,
            following.len(),
            followers,
        ))
    }

    /// The newest `limit` casts from the accounts `fid` follows that `apps`
    /// admits. Each account's newest `limit` are read, so a filter can
    /// leave fewer.
    pub async fn feed(
        &self,
        fid: u64,
        limit: u32,
        apps: &AppFilter,
        now: Timestamp,
    ) -> Result<Vec<FeedItem>, Error> {
        let follows = self.all_follows_by_fid(fid).await?;
        self.newest_casts(follows, limit, apps, now).await
    }

    /// The newest `limit` casts across `fids` that `apps` admits, read as
    /// for [`Node::feed`].
    pub async fn newest_casts(
        &self,
        fids: Vec<u64>,
        limit: u32,
        apps: &AppFilter,
        now: Timestamp,
    ) -> Result<Vec<FeedItem>, Error> {
        let (casts, signers) = self.casts_and_signers(fids, &page_of(limit)).await?;
        Ok(feed::assemble(casts, &signers, apps, now, limit as usize))
    }

    /// The newest `limit` casts in `channel` that `apps` admits, from the
    /// channel's newest `limit` on each shard.
    pub async fn channel(
        &self,
        channel: &ChannelUrl,
        limit: u32,
        apps: &AppFilter,
        now: Timestamp,
    ) -> Result<Vec<FeedItem>, Error> {
        let (casts, signers) = self.channel_casts(channel, &page_of(limit)).await?;
        Ok(feed::assemble(casts, &signers, apps, now, limit as usize))
    }

    /// The newest `limit` replies to `parent` that `apps` admits, oldest
    /// first. Every reply is read before filtering.
    pub async fn newest_replies(
        &self,
        parent: &CastRef,
        limit: u32,
        apps: &AppFilter,
        now: Timestamp,
    ) -> Result<Vec<FeedItem>, Error> {
        let casts = self.replies(parent).await?;
        let signers = self
            .signers_of(casts.iter().map(|c| c.id.fid).collect())
            .await?;
        let mut items = feed::assemble(casts, &signers, apps, now, limit as usize);
        items.reverse();
        Ok(items)
    }

    /// A read of the events in blocks at or after `since` and before
    /// `until`, on `shard` or on every shard with events. A shard the node
    /// does not have is an error.
    pub async fn event_read(
        &self,
        shard: Option<u32>,
        since: Option<Timestamp>,
        until: Option<Timestamp>,
    ) -> Result<EventRead, Error> {
        let shards: VecDeque<(u32, u64)> = self
            .sync_state()
            .await?
            .shards
            .iter()
            .filter(|s| s.id != 0 && shard.is_none_or(|n| n == s.id))
            .map(|s| (s.id, s.height))
            .collect();
        match shard {
            Some(n) if shards.is_empty() => Err(Error::NoShard(n)),
            _ => Ok(EventRead {
                shards,
                since,
                until,
                walk: None,
            }),
        }
    }

    /// The next page of `read`'s events, which may be empty; `None` once
    /// every shard is read.
    pub async fn next_events(&self, read: &mut EventRead) -> Result<Option<Vec<Event>>, Error> {
        loop {
            if let Some(walk) = &mut read.walk
                && let Some(from) = walk.next_id()
            {
                let page = self.events(walk.shard(), from, EVENT_PAGE_SIZE).await?;
                return walk.add_page(page).map(Some);
            }
            let Some((shard, height)) = read.shards.pop_front() else {
                return Ok(None);
            };
            let start = match read.since {
                Some(t) => self.first_event_since(shard, t, height).await?,
                None => 0,
            };
            read.walk = Some(EventWalk::new(shard, start, read.since, read.until));
        }
    }

    /// The cast `id`, with the app it was posted from, and the user data
    /// of its author and the accounts it mentions. A cast the node does not
    /// hold is an error.
    pub async fn cast(&self, id: &CastRef) -> Result<(FeedItem, Users), Error> {
        let cast = self.cast_by_id(id.fid, &id.hash).await?;
        let signers = self.signers_by_fid(id.fid).await?;
        let item = FeedItem::attributed(cast, &signers);
        let users = self.accounts_of(std::slice::from_ref(&item)).await?;
        Ok((item, users))
    }

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

    /// The likes and recasts of `id`, and the user data of everyone who
    /// reacted. A cast the node does not hold is an error.
    pub async fn reactions(&self, id: &CastRef) -> Result<(CastReactions, Users), Error> {
        self.cast_by_id(id.fid, &id.hash).await?;
        let (likes, recasts) = future::try_join(
            self.reactions_to_cast(id, ReactionType::Like),
            self.reactions_to_cast(id, ReactionType::Recast),
        )
        .await?;
        let reactions = CastReactions::assemble(likes, recasts);
        let users = self.user_data_of(reactions.account_fids()).await?;
        Ok((reactions, users))
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

    /// The newest casts posted in `channel`, `page`'s size from each shard,
    /// and their authors' app keys.
    pub async fn channel_casts(
        &self,
        channel: &ChannelUrl,
        page: &PageRequest,
    ) -> Result<(Vec<Cast>, Vec<Signer>), Error> {
        let casts = self.casts_in_channel(channel, page).await?.items;
        let signers = self
            .signers_of(casts.iter().map(|c| c.id.fid).collect())
            .await?;
        Ok((casts, signers))
    }

    /// The newest `limit` accounts `fid` follows (`followers` false) or
    /// that follow it (`true`), and their user data. An account the node
    /// has no registration for is an error.
    pub async fn follow_list(
        &self,
        fid: u64,
        followers: bool,
        limit: u32,
    ) -> Result<(Vec<Follow>, Users), Error> {
        if !self.is_registered(fid).await? {
            return Err(Error::NotRegistered(fid));
        }
        let follows = match followers {
            true => self.newest_followers(fid, limit).await?,
            false => self.newest_follows(fid, limit).await?,
        };
        let users = self
            .user_data_of(follows.iter().map(|f| f.fid).collect())
            .await?;
        Ok((follows, users))
    }

    /// The addresses `fid` has verified, oldest first, with its primary
    /// ones marked. An account the node has no registration for is an
    /// error.
    pub async fn addresses(&self, fid: u64) -> Result<Vec<Address>, Error> {
        if !self.is_registered(fid).await? {
            return Err(Error::NotRegistered(fid));
        }
        let (verifications, user_data) =
            future::try_join(self.verifications_by_fid(fid), self.user_data_by_fid(fid)).await?;
        Ok(address::assemble(verifications, &user_data))
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

    /// The FID `account` names, looking a username up on the node.
    pub async fn resolve(&self, account: &Account) -> Result<u64, Error> {
        match account {
            Account::Fid(fid) => Ok(*fid),
            Account::Name(name) => self.fid_of_name(name).await,
        }
    }

    /// The FID of each of `accounts`, in the same order.
    pub async fn resolve_all(&self, accounts: &[Account]) -> Result<Vec<u64>, Error> {
        stream::iter(accounts)
            .map(|a| self.resolve(a))
            .buffered(FETCH_AT_ONCE)
            .try_collect()
            .await
    }

    /// The user data of each author of `items` and each account they
    /// mention.
    pub async fn accounts_of(&self, items: &[FeedItem]) -> Result<Users, Error> {
        self.user_data_of(feed::account_fids(items)).await
    }
}

fn page_of(limit: u32) -> PageRequest {
    PageRequest {
        page_size: Some(limit),
        page_token: None,
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
