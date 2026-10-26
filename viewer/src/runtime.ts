// What a page built on Paircaster can use: the reads, named after the CLI and
// returning what `paircaster <read> --json` prints, run in the page through
// paircaster-core compiled to WebAssembly. Reads only: a page holds no keys
// and cannot write.
//
// The viewer is the first page built on this. A page an agent writes would
// use the same object, found at `window.paircaster`.

import wasmBase64 from "../wasm/module";
import * as wasm from "../wasm/paircaster_wasm.js";
import type { Addresses, Casts, Channel, Event, Feed, Followers, Following, Profile, Reactions, Replies, Show } from "./reads";
import { versioned } from "./reads";
import { asThread, type Thread } from "./thread";

// A FID, or a username as the CLI takes it: `dwr`, `vitalik.eth`, `@123`.
export type AccountName = number | string;

// The CLI's `--include-app` and `--exclude-app`, by app FID. Not both.
export type AppOptions = { includeApp?: number[]; excludeApp?: number[] };
export type CastOptions = { limit?: number } & AppOptions;

export type Paircaster = {
  // The node the page reads.
  node: string;
  // `paircaster profile <account> [--all-followers]`.
  profile(account: AccountName, options?: { allFollowers?: boolean }): Promise<Profile>;
  // `paircaster feed <account>`; limit 1–1000, default 20.
  feed(account: AccountName, options?: CastOptions): Promise<Feed>;
  // `paircaster casts <account>,…`; limit 1–1000, default 20.
  casts(accounts: AccountName | AccountName[], options?: CastOptions): Promise<Casts>;
  // `paircaster channel <url>`; limit 1–1000, default 20.
  channel(url: string, options?: CastOptions): Promise<Channel>;
  // `paircaster addresses <account>`.
  addresses(account: AccountName): Promise<Addresses>;
  // `paircaster followers <account>`; limit 1–10000, default 100.
  followers(account: AccountName, options?: { limit?: number }): Promise<Followers>;
  // `paircaster following <account>`; limit 1–10000, default 100.
  following(account: AccountName, options?: { limit?: number }): Promise<Following>;
  // `paircaster replies <fid:hash>`; limit 1–1000, default 100.
  replies(cast: string, options?: CastOptions): Promise<Replies>;
  // `paircaster thread <fid:hash>`; limit 1–1000, default 100.
  thread(cast: string, options?: { limit?: number }): Promise<Thread>;
  // `paircaster show <fid:hash>`: one cast. A cast the node does not hold
  // rejects.
  show(cast: string): Promise<Show>;
  // `paircaster reactions <fid:hash>`.
  reactions(cast: string): Promise<Reactions>;
  // `paircaster events`, one event at a time, read a page of up to 10000
  // underneath; `since` and `until` as the CLI takes them (2026-10-22 or
  // 2026-10-22T06:00:00Z). Leaving the loop early stops the reading.
  events(options?: { since?: string; until?: string; shard?: number }): AsyncIterable<Event>;
};

let loaded = false;

function load() {
  if (loaded) return;
  const bytes = Uint8Array.from(atob(wasmBase64), (c) => c.charCodeAt(0));
  wasm.initSync({ module: bytes });
  loaded = true;
}

const nameOf = (account: AccountName) => String(account);
const listOf = (a: AccountName | AccountName[]) => (Array.isArray(a) ? a : [a]).map(nameOf);
const apps = (o: AppOptions) => [Float64Array.from(o.includeApp ?? []), Float64Array.from(o.excludeApp ?? [])] as const;

export function makeRuntime(node: string): Paircaster {
  load();
  const now = () => Math.floor(Date.now() / 1000);
  const paircaster: Paircaster = {
    node,
    profile: async (account, o = {}) => versioned(await wasm.profile(node, nameOf(account), o.allFollowers ?? false), "profile"),
    feed: async (account, o = {}) => versioned(await wasm.feed(node, nameOf(account), o.limit ?? 20, ...apps(o), now()), "feed"),
    casts: async (accounts, o = {}) => versioned(await wasm.casts(node, listOf(accounts), o.limit ?? 20, ...apps(o), now()), "casts"),
    channel: async (url, o = {}) => versioned(await wasm.channel(node, url, o.limit ?? 20, ...apps(o), now()), "channel"),
    addresses: async (account) => versioned(await wasm.addresses(node, nameOf(account)), "addresses"),
    followers: async (account, o = {}) => versioned(await wasm.followers(node, nameOf(account), o.limit ?? 100), "followers"),
    following: async (account, o = {}) => versioned(await wasm.following(node, nameOf(account), o.limit ?? 100), "following"),
    replies: async (cast, o = {}) => versioned(await wasm.replies(node, cast, o.limit ?? 100, ...apps(o), now()), "replies"),
    thread: async (cast, o = {}) => asThread(JSON.parse(await wasm.thread(node, cast, o.limit ?? 100, now()))),
    show: async (cast) => versioned(await wasm.show(node, cast), "show"),
    reactions: async (cast) => versioned(await wasm.reactions(node, cast), "reactions"),
    events: (o = {}) => events(node, o),
  };
  (window as any).paircaster = paircaster;
  return paircaster;
}

async function* events(node: string, o: { since?: string; until?: string; shard?: number }): AsyncGenerator<Event> {
  const read = await wasm.events(node, o.shard, o.since, o.until);
  try {
    for (let page = await read.next(); page !== undefined; page = await read.next()) {
      for (const e of JSON.parse(page) as Event[]) {
        if (e.version !== 1) throw new Error("not a version 1 event");
        yield e;
      }
    }
  } finally {
    read.free();
  }
}
