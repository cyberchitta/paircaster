//! A node's state once it answers: syncing or ready, and how far behind each
//! shard is. `paircaster node status` and `paircaster-node status` both
//! report it, in the same words and the same `--json` (version 1) fields.

use serde::Serialize;

use crate::node::SyncState;

/// A node is ready when every shard's newest block is at most this old.
pub const READY_WITHIN_SECS: u64 = 60;

/// The answering phases of `node status --json`, version 1, flattened into
/// its object: `{ "phase": "syncing", "node_version": "0.15.0", "shards":
/// [{ "id": 0, "height": 48340352, "seconds_behind": 4200 }] }`. `ready`
/// has the same fields.
#[derive(Debug, Serialize)]
#[serde(tag = "phase", rename_all = "snake_case")]
pub enum Synced {
    Syncing {
        node_version: String,
        shards: Vec<ShardV1>,
    },
    Ready {
        node_version: String,
        shards: Vec<ShardV1>,
    },
}

#[derive(Debug, Serialize)]
pub struct ShardV1 {
    pub id: u32,
    pub height: u64,
    pub seconds_behind: u64,
}

impl Synced {
    /// Ready only when the node reports shards and every one is within
    /// [`READY_WITHIN_SECS`].
    pub fn from_state(state: SyncState) -> Self {
        let ready = state
            .seconds_behind()
            .is_some_and(|s| s <= READY_WITHIN_SECS);
        let shards = state
            .shards
            .iter()
            .map(|s| ShardV1 {
                id: s.id,
                height: s.height,
                seconds_behind: s.seconds_behind,
            })
            .collect();
        let node_version = state.version;
        match ready {
            true => Self::Ready {
                node_version,
                shards,
            },
            false => Self::Syncing {
                node_version,
                shards,
            },
        }
    }

    /// The human-readable report: a headline, then one line per shard.
    pub fn describe(&self) -> String {
        match self {
            Self::Syncing {
                node_version,
                shards,
            } => format!(
                "{}\n{}",
                syncing_head(node_version, shards),
                shard_lines(shards)
            ),
            Self::Ready {
                node_version,
                shards,
            } => format!(
                "Ready (Snapchain {node_version}): every shard within a minute of its newest block.\n{}",
                shard_lines(shards)
            ),
        }
    }
}

/// How far behind the shards that hold accounts are. Shard 0 holds the
/// blocks and no account's casts, reactions, follows or profile, which the
/// node serves from shards 1 and up (snapchain v0.14.3
/// `src/network/server.rs`, `get_stores_for`), so its lag is left out
/// unless it is the only one.
fn syncing_head(node_version: &str, shards: &[ShardV1]) -> String {
    let behind = |block_shard: bool| {
        shards
            .iter()
            .filter(|s| (s.id == 0) == block_shard)
            .map(|s| s.seconds_behind)
            .max()
    };
    match (behind(false), behind(true)) {
        (Some(accounts), Some(blocks)) if accounts <= READY_WITHIN_SECS => format!(
            "Syncing (Snapchain {node_version}): the shards that hold accounts are within a minute of their newest block; shard 0, which holds the blocks, is {} behind.",
            duration(blocks)
        ),
        (accounts, blocks) => format!(
            "Syncing (Snapchain {node_version}): {} behind at most.",
            duration(accounts.or(blocks).unwrap_or(0))
        ),
    }
}

fn shard_lines(shards: &[ShardV1]) -> String {
    shards
        .iter()
        .map(|s| {
            format!(
                "  shard {}: height {}, {} behind",
                s.id,
                s.height,
                duration(s.seconds_behind)
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// `3h12m`, `4m`, `45s`.
///
/// ```
/// assert_eq!(paircaster_core::status::duration(3 * 3600 + 12 * 60 + 5), "3h12m");
/// ```
pub fn duration(secs: u64) -> String {
    match (secs / 3600, secs % 3600 / 60) {
        (0, 0) => format!("{secs}s"),
        (0, m) => format!("{m}m"),
        (h, m) => format!("{h}h{m:02}m"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::ShardSync;

    fn state(behind: &[u64]) -> SyncState {
        SyncState {
            version: "0.14.3".to_owned(),
            shards: (0..)
                .zip(behind)
                .map(|(id, &seconds_behind)| ShardSync {
                    id,
                    height: 1,
                    seconds_behind,
                })
                .collect(),
        }
    }

    #[test]
    fn ready_only_when_every_shard_is_within_a_minute() {
        let synced = |behind: &[u64]| Synced::from_state(state(behind));
        assert!(matches!(synced(&[1, 60, 0]), Synced::Ready { .. }));
        assert!(matches!(synced(&[1, 61, 0]), Synced::Syncing { .. }));
        assert!(matches!(synced(&[]), Synced::Syncing { .. }));
    }

    fn head(behind: &[u64]) -> String {
        let shards: Vec<ShardV1> = (0..)
            .zip(behind)
            .map(|(id, &seconds_behind)| ShardV1 {
                id,
                height: 1,
                seconds_behind,
            })
            .collect();
        syncing_head("0.14.3", &shards)
    }

    #[test]
    fn the_lag_quoted_leaves_out_the_block_shard() {
        assert_eq!(
            head(&[85_740, 3, 50]),
            "Syncing (Snapchain 0.14.3): the shards that hold accounts are within a minute of their newest block; shard 0, which holds the blocks, is 23h49m behind."
        );
        assert_eq!(
            head(&[169_620, 7_200, 3]),
            "Syncing (Snapchain 0.14.3): 2h00m behind at most."
        );
        assert_eq!(
            head(&[85_740, 150_900, 169_620]),
            "Syncing (Snapchain 0.14.3): 47h07m behind at most."
        );
        assert_eq!(
            head(&[200]),
            "Syncing (Snapchain 0.14.3): 3m behind at most."
        );
    }

    #[test]
    fn json_flattens_the_phase_and_its_fields() {
        let json = serde_json::to_string(&Synced::from_state(state(&[5]))).expect("serialises");
        assert_eq!(
            json,
            r#"{"phase":"ready","node_version":"0.14.3","shards":[{"id":0,"height":1,"seconds_behind":5}]}"#
        );
    }

    #[test]
    fn durations_read_as_hours_minutes_or_seconds() {
        assert_eq!(duration(45), "45s");
        assert_eq!(duration(240), "4m");
        assert_eq!(duration(3 * 3600 + 12 * 60 + 5), "3h12m");
        assert_eq!(duration(3600 + 5 * 60), "1h05m");
    }
}
