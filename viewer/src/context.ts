// What a card shows of the casts around it: each cast it quotes, as a small
// card, and in a list the cast it replies to, as one line above it. A thread
// draws a reply's parent right above it, so a thread shows quotes only.
//
// The page reads a context cast once per node (`show`), unless it already
// holds it: a cast drawn, folded or read below another, or a list cast's
// parent, which the list read already walked up to.

import { type Account, type Cast, refOf, type Thread } from "./thread";

// A context cast and its author as the page knows them; `null` for one the
// node does not hold (deleted, or pruned).
export interface Context {
  cast: Cast;
  account: Account | undefined;
}
export type Contexts = ReadonlyMap<string, Context | null>;

// How much of a context cast's text shows: a quote's, clamped to three lines
// by viewer.css, and a parent's, on one line.
export const QUOTE_EXCERPT = 280;
export const PARENT_EXCERPT = 140;

export const quotesOf = (c: Cast): string[] => c.embeds.flatMap((e) => ("cast" in e ? [refOf(e.cast)] : []));

// The cast a cast replies to; none for a cast in a channel, which replies to
// a URL.
export const parentOf = (c: Cast): string | null => (c.parent && "cast" in c.parent ? refOf(c.parent.cast) : null);

// The context casts the cards drawn need, each once, in page order: each
// card's parent (in a list) and then what it quotes.
export function wantedOf(drawn: readonly Cast[], list: boolean): string[] {
  const refs = drawn.flatMap((c) => [...(list ? [parentOf(c)] : []), ...quotesOf(c)]);
  return [...new Set(refs.filter((r): r is string => r !== null))];
}

// The casts the page holds already, as context, with their authors.
export function heldOf(casts: readonly Cast[], accounts: readonly Account[]): Map<string, Context> {
  const byFid = new Map(accounts.map((a) => [a.fid, a]));
  return new Map(casts.map((c) => [refOf(c), { cast: c, account: byFid.get(c.fid) }]));
}

// Each list cast's parent, from the thread the list read for it: the
// nearest cast above, or `null` where the node does not hold the parent.
export function parentsOf(threads: Thread[]): Map<string, Context | null> {
  const out = new Map<string, Context | null>();
  for (const t of threads) {
    const parent = parentOf(t.cast);
    if (!parent) continue;
    const up = t.ancestors.at(-1);
    if (up && refOf(up) === parent) out.set(parent, { cast: up, account: t.accounts.find((a) => a.fid === up.fid) });
    else if (typeof t.top === "object" && "missing" in t.top && refOf(t.top.missing) === parent) out.set(parent, null);
  }
  return out;
}

// A cast's text on one line, cut at a word near `max` characters.
export function excerpt(cast: Cast, max: number): string {
  const chars = Array.from((cast.display_text ?? cast.text).replace(/\s+/g, " ").trim());
  if (chars.length <= max) return chars.join("");
  const cut = chars.slice(0, max).join("");
  const space = cut.lastIndexOf(" ");
  return `${(space > max * 0.6 ? cut.slice(0, space) : cut).trimEnd()}…`;
}

// Who wrote a context cast, as a reply names them.
export function authorOf({ cast, account }: Context): string {
  return account?.username ? `@${account.username}` : (account?.display_name ?? `fid ${cast.fid}`);
}
