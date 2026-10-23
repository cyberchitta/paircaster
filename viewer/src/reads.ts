// The results of the reads other than `thread`: what `selfcast <read>
// --json` prints, version 1 (crates/core/src/output.rs, which documents
// each field).

import type { Account, Cast, Ref, Target } from "./thread";

export type Profile = {
  version: 1;
  fid: number;
  username: string | null;
  display_name: string | null;
  bio: string | null;
  pfp_url: string | null;
  url: string | null;
  location: string | null;
  twitter: string | null;
  github: string | null;
  banner_url: string | null;
  following: number;
  followers: number;
  followers_exact: boolean;
};

export type Feed = { version: 1; fid: number; casts: Cast[]; accounts: Account[] };
export type Casts = { version: 1; fids: number[]; casts: Cast[]; accounts: Account[] };
export type Channel = { version: 1; channel: string; casts: Cast[]; accounts: Account[] };
export type Replies = { version: 1; parent: Ref; casts: Cast[]; accounts: Account[] };

// An account and when it followed, liked or recast.
export type AccountAt = { fid: number; timestamp: string };

export type Followers = { version: 1; fid: number; followers: AccountAt[]; accounts: Account[] };
export type Following = { version: 1; fid: number; following: AccountAt[]; accounts: Account[] };

export type Addresses = {
  version: 1;
  fid: number;
  addresses: {
    address: string;
    protocol: "ethereum" | "solana";
    chain_id: number;
    timestamp: string;
    primary: boolean;
  }[];
};

export type Reactions = {
  version: 1;
  cast: Ref;
  likes: AccountAt[];
  recasts: AccountAt[];
  accounts: Account[];
};

type EventHead = { version: 1; shard: number; id: number; block: number; block_time: string };

export type Event = EventHead &
  (
    | {
        event: "merge" | "prune" | "revoke";
        type: string;
        fid: number;
        hash: string;
        timestamp: string;
        signer: string;
        // A cast_add's cast, without usernames; absent for other types.
        text?: string;
        mentions?: { fid: number; position: number }[];
        embeds?: Target[];
        parent?: Target | null;
      }
    | { event: "username_proof"; type: string; fid: number; name: string }
    | { event: "onchain"; type: string; fid: number }
    | { event: "other"; type: string }
  );

// Parses a read's JSON and checks it is the version this page knows.
export function versioned<T>(json: string, read: string): T {
  const value = JSON.parse(json);
  if (value?.version !== 1) throw new Error(`not a version 1 ${read} result`);
  return value as T;
}
