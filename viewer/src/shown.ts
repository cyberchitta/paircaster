// What a spot reads from the node, and which of it the page shows.

import { BRANCH_READ, type Branch, branchOf } from "./branch";
import { type Context, type Contexts, heldOf, parentsOf } from "./context";
import { continuedOf, drawnOf, foldedOf, layout, type Nest } from "./fold";
import type { Spot } from "./moniker";
import type { Profile } from "./reads";
import type { Paircaster } from "./runtime";
import { type Account, type Cast, refOf, type Thread } from "./thread";

// What a spot read from the node. A list keeps each cast's parent (`null`
// where the node does not hold it), which its read walked up to anyway. A
// profile is the account and its latest casts, a list too, whose parents the
// page reads after it draws.
export type Shown =
  | { kind: "thread"; thread: Thread; only: string[] | null }
  | { kind: "casts"; casts: Cast[]; accounts: Account[]; parents: Map<string, Context | null> }
  | { kind: "profile"; profile: Profile; casts: Cast[]; accounts: Account[] };

// How many of an account's casts its profile shows.
export const PROFILE_CASTS = 25;

// A cast's thread, with any failure naming the cast, so a refused list says
// which of its casts the node could not give.
function threadOf(runtime: Paircaster, cast: string, options?: { limit?: number }): Promise<Thread> {
  return runtime.thread(cast, options).catch((e: unknown) => {
    throw new Error(`${cast}: ${e instanceof Error ? e.message : String(e)}`);
  });
}

// `known` is the account's profile when the page holds it already.
export async function read(runtime: Paircaster, spot: Spot, known?: Profile): Promise<Shown> {
  switch (spot.kind) {
    case "profile":
      return readProfile(runtime, spot.fid, known);
    case "casts":
      return readCasts(runtime, spot.casts);
    case "thread":
      return readThread(runtime, spot.cast, spot.only);
    default: {
      const unknown: never = spot;
      throw new Error(`no read for ${JSON.stringify(unknown)}`);
    }
  }
}

async function readProfile(runtime: Paircaster, fid: number, known?: Profile): Promise<Shown> {
  const [profile, { casts, accounts }] = await Promise.all([known ?? runtime.profile(fid), runtime.casts(fid, { limit: PROFILE_CASTS })]);
  return { kind: "profile", profile, casts, accounts };
}

async function readCasts(runtime: Paircaster, refs: readonly string[]): Promise<Shown> {
  const threads = await Promise.all(refs.map((c) => threadOf(runtime, c, { limit: 1 })));
  const accounts = new Map(threads.flatMap((t) => t.accounts).map((a) => [a.fid, a]));
  const casts = threads.map((t) => t.cast);
  return { kind: "casts", casts, accounts: [...accounts.values()], parents: parentsOf(threads) };
}

async function readThread(runtime: Paircaster, cast: string, only: string[] | null): Promise<Shown> {
  const thread = await threadOf(runtime, cast);
  const stray = only?.find((r) => !thread.replies.some((c) => refOf(c) === r));
  if (stray) throw new Error(`${stray} is not a reply to ${cast}`);
  return { kind: "thread", thread, only };
}

// The casts the spot's own read holds, drawn or folded: with `only`, the
// picks in their order and then the other replies. Nested replies are not
// among them: the page reads those after it draws.
export function visible(shown: Shown): Cast[] {
  if (shown.kind !== "thread") return shown.casts;
  const { ancestors, cast, replies } = shown.thread;
  const only = shown.only;
  if (!only) return [...ancestors, cast, ...replies];
  const picks = only.flatMap((r) => replies.find((c) => refOf(c) === r) ?? []);
  return [...ancestors, cast, ...picks, ...replies.filter((c) => !only.includes(refOf(c)))];
}

// The casts drawn as cards, those folded, and those whose replies continue
// in their own thread, given the folds the owner opened, the casts marked
// and the replies read. A list folds and nests nothing.
export function onScreen(shown: Shown, opened: ReadonlySet<string>, marked: string[], nest: Nest) {
  if (shown.kind !== "thread") return { drawn: shown.casts, folded: [], continued: [] };
  const { thread, only } = shown;
  const l = layout(thread, only, opened, marked, nest);
  const drawn = [...drawnOf({ ...l, replies: [] }), thread.cast, ...drawnOf({ ...l, above: [] })];
  return { drawn, folded: foldedOf(l), continued: continuedOf(l) };
}

// Every account the page can name: the read's, and those of the replies read.
export function accountsOf(shown: Shown, nest: Nest): Account[] {
  const own = shown.kind === "thread" ? shown.thread.accounts : shown.accounts;
  const all = new Map([...[...nest.branches.values()].flatMap((b) => b.accounts), ...own].map((a) => [a.fid, a]));
  return [...all.values()];
}

// The context casts the page can draw: those it read with `show`, and over
// them every cast it holds for another reason, so a read never repeats one.
export function contextsOf(shown: Shown, nest: Nest, read: Contexts): Contexts {
  const branched = [...nest.branches.values()].flatMap((b) => b.casts);
  const held = heldOf([...visible(shown), ...branched], accountsOf(shown, nest));
  const parents = shown.kind === "casts" ? shown.parents : new Map();
  return new Map([...read, ...parents, ...held]);
}

// The replies on the way from a thread's focus down to `mark`, a cast nested
// under one of its replies, read so the page can draw it marked; throws when
// `mark` is not under the focus or a reply on the way is not among the
// newest its parent's read holds.
export async function reach(runtime: Paircaster, thread: Thread, mark: string, known: ReadonlyMap<string, Branch>): Promise<Map<string, Branch>> {
  const missing = new Error(`${mark} is not on the page the thread of ${refOf(thread.cast)} shows`);
  const above = (await threadOf(runtime, mark, { limit: 1 })).ancestors.map(refOf);
  const path = above.slice(above.indexOf(refOf(thread.cast)) + 1);
  if (!above.includes(refOf(thread.cast)) || !thread.replies.some((c) => refOf(c) === path[0])) throw missing;
  const out = new Map<string, Branch>();
  for (const [i, ref] of path.entries()) {
    const branch = known.get(ref) ?? branchOf(await runtime.replies(ref, { limit: BRANCH_READ }));
    const next = path[i + 1] ?? mark;
    if (!branch.casts.some((c) => refOf(c) === next)) throw missing;
    out.set(ref, branch);
  }
  return out;
}

export const count = (n: number, one: string, many = `${one}s`) => `${n} ${n === 1 ? one : many}`;
