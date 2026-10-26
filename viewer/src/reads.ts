// The results of the reads other than `thread`: what `paircaster <read>
// --json` prints, version 1 (crates/core/src/output.rs, which documents
// each field).

import type { Account, Cast, Ref, Target } from "./thread";

export type Profile = {
  readonly version: 1;
  readonly fid: number;
  readonly username: string | null;
  readonly display_name: string | null;
  readonly bio: string | null;
  readonly pfp_url: string | null;
  readonly url: string | null;
  readonly location: string | null;
  readonly twitter: string | null;
  readonly github: string | null;
  readonly banner_url: string | null;
  readonly following: number;
  readonly followers: number;
  readonly followers_exact: boolean;
};

export type Feed = { readonly version: 1; readonly fid: number; readonly casts: readonly Cast[]; readonly accounts: readonly Account[] };
export type Casts = { readonly version: 1; readonly fids: readonly number[]; readonly casts: readonly Cast[]; readonly accounts: readonly Account[] };
export type Channel = { readonly version: 1; readonly channel: string; readonly casts: readonly Cast[]; readonly accounts: readonly Account[] };
export type Replies = { readonly version: 1; readonly parent: Ref; readonly casts: readonly Cast[]; readonly accounts: readonly Account[] };
// One cast by its ID, without the casts around it.
export type Show = { readonly version: 1; readonly cast: Cast; readonly accounts: readonly Account[] };

// An account and when it followed, liked or recast.
export type AccountAt = { readonly fid: number; readonly timestamp: string };

export type Followers = { readonly version: 1; readonly fid: number; readonly followers: readonly AccountAt[]; readonly accounts: readonly Account[] };
export type Following = { readonly version: 1; readonly fid: number; readonly following: readonly AccountAt[]; readonly accounts: readonly Account[] };

export type Addresses = {
  readonly version: 1;
  readonly fid: number;
  readonly addresses: readonly {
    readonly address: string;
    readonly protocol: "ethereum" | "solana";
    readonly chain_id: number;
    readonly timestamp: string;
    readonly primary: boolean;
  }[];
};

export type Reactions = {
  readonly version: 1;
  readonly cast: Ref;
  readonly likes: readonly AccountAt[];
  readonly recasts: readonly AccountAt[];
  readonly accounts: readonly Account[];
};

type EventHead = { readonly version: 1; readonly shard: number; readonly id: number; readonly block: number; readonly block_time: string };

export type Event = EventHead &
  (
    | {
        readonly event: "merge" | "prune" | "revoke";
        readonly type: string;
        readonly fid: number;
        readonly hash: string;
        readonly timestamp: string;
        readonly signer: string;
        // A cast_add's cast, without usernames; absent for other types.
        readonly text?: string;
        readonly mentions?: readonly { readonly fid: number; readonly position: number }[];
        readonly embeds?: readonly Target[];
        readonly parent?: Target | null;
      }
    | { readonly event: "username_proof"; readonly type: string; readonly fid: number; readonly name: string }
    | { readonly event: "onchain"; readonly type: string; readonly fid: number }
    | { readonly event: "other"; readonly type: string }
  );

// Parses a read's JSON and checks it is the version this page knows. The
// rest is cast, not parsed: the JSON is paircaster-core's own output, built
// into this page as WebAssembly, which parsed the node's responses into
// these shapes before printing them. A build whose core prints another
// shape changes the version.
export function versioned<T>(json: string, read: string): T {
  const value = JSON.parse(json);
  if (value?.version !== 1) throw new Error(`not a version 1 ${read} result`);
  return value as T;
}
