// What a spot reads from the node, and which of it the page shows.

import type { Spot } from "./moniker";
import type { Paircaster } from "./runtime";
import { type Account, type Cast, type Thread, refOf } from "./thread";

// What a spot read from the node.
export type Shown =
  | { kind: "thread"; thread: Thread; only: string[] | null }
  | { kind: "casts"; casts: Cast[]; accounts: Account[] };

// A cast's thread, with any failure naming the cast, so a refused list says
// which of its casts the node could not give.
function threadOf(runtime: Paircaster, cast: string, options?: { limit?: number }): Promise<Thread> {
  return runtime.thread(cast, options).catch((e: unknown) => {
    throw new Error(`${cast}: ${e instanceof Error ? e.message : String(e)}`);
  });
}

export async function read(runtime: Paircaster, spot: Spot): Promise<Shown> {
  if (spot.kind === "casts") {
    const threads = await Promise.all(spot.casts.map((c) => threadOf(runtime, c, { limit: 1 })));
    const accounts = new Map(threads.flatMap((t) => t.accounts).map((a) => [a.fid, a]));
    return { kind: "casts", casts: threads.map((t) => t.cast), accounts: [...accounts.values()] };
  }
  const thread = await threadOf(runtime, spot.cast);
  const stray = spot.only?.find((r) => !thread.replies.some((c) => refOf(c) === r));
  if (stray) throw new Error(`${stray} is not a reply to ${spot.cast}`);
  return { kind: "thread", thread, only: spot.only };
}

// The casts on the page, in the order shown.
export function visible(shown: Shown): Cast[] {
  if (shown.kind === "casts") return shown.casts;
  const { ancestors, cast } = shown.thread;
  return [...ancestors, cast, ...repliesShown(shown.thread, shown.only)];
}

export const repliesShown = (thread: Thread, only: string[] | null) =>
  only ? only.map((r) => thread.replies.find((c) => refOf(c) === r)!) : thread.replies;

export const accountsOf = (shown: Shown) => (shown.kind === "thread" ? shown.thread.accounts : shown.accounts);

export const count = (n: number, one: string, many = one + "s") => `${n} ${n === 1 ? one : many}`;
