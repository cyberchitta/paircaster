//! Items gathered from paged reads whose pages repeat, each kept once.

use std::collections::HashSet;
use std::hash::Hash;

/// Items gathered from pages, each kept once by its key.
///
/// Some of the node's paged reads repeat: `castsByParent` and
/// `reactionsByCast` (snapchain v0.14.3). A page holds items from every
/// message shard, and a shard that has run out starts again from its first
/// item on the next page; once all have run out, the next page is the first
/// one again. The node never ends the paging, so an item can arrive on
/// several pages, and the read is complete at the first page that brings
/// nothing new.
///
/// ```
/// use selfcast_core::tally::PageTally;
/// let mut tally = PageTally::default();
/// assert!(tally.add_page(vec![1, 2], |n: &u32| *n));
/// assert!(tally.add_page(vec![2, 3], |n: &u32| *n));
/// assert!(!tally.add_page(vec![1, 3], |n: &u32| *n));
/// assert_eq!(tally.into_items(), [1, 2, 3]);
/// ```
#[derive(Debug, Clone)]
pub struct PageTally<K, T> {
    seen: HashSet<K>,
    items: Vec<T>,
}

impl<K, T> Default for PageTally<K, T> {
    fn default() -> Self {
        Self {
            seen: HashSet::new(),
            items: Vec::new(),
        }
    }
}

impl<K: Eq + Hash, T> PageTally<K, T> {
    /// Adds one page, skipping any item whose key was already seen; `false`
    /// when none of them is new.
    pub fn add_page(&mut self, page: Vec<T>, key: impl Fn(&T) -> K) -> bool {
        let before = self.items.len();
        let fresh: Vec<T> = page
            .into_iter()
            .filter(|item| self.seen.insert(key(item)))
            .collect();
        self.items.extend(fresh);
        self.items.len() > before
    }

    /// Every item seen, in the order the pages gave them.
    pub fn into_items(self) -> Vec<T> {
        self.items
    }
}
