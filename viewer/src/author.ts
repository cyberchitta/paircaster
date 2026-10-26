// The author card's logic, apart from the page: what its counts say, where it
// goes beside the name or picture it describes, and when the page reads a
// profile for it.
//
// The card shows the account as `profile` reads it (reads.ts), once per FID
// per node. That read counts followers up to 10,000, a page of 1000 at a
// time, so a large account costs about a dozen node requests: the page waits
// a moment on a hover before reading, so a sweep of the mouse reads nothing.

import type { Profile } from "./reads";
import type { Account } from "./thread";

// How long the pointer rests on an author before the card opens and the page
// reads the profile, and how long the card stays once the pointer leaves, so
// it can cross onto the card.
export const HOVER_DELAY = 400;
export const CLOSE_DELAY = 150;

// The gap between the card and what it describes, and the space it keeps
// from the pane's edges.
export const GAP = 6;
export const MARGIN = 8;

// What the page holds of each account's profile, read on this node; `null`
// for one the node refused.
export type Profiles = ReadonlyMap<number, Profile | null>;

export const profileJob = (fid: number) => `profile ${fid}`;

// Whether a hover on `fid` reads its profile: not when the page holds it, or
// asked for it already on this view.
export const wanted = (profiles: Profiles, asked: ReadonlySet<string>, fid: number): boolean => !profiles.has(fid) && !asked.has(profileJob(fid));

// The account as the card names it: the profile once read, else what the
// page holds from the casts.
export function shownAccount(fid: number, account: Account | undefined, profile: Profile | null | undefined): Account {
  if (!profile) return account ?? { fid, username: null, display_name: null, pfp_url: null };
  const { username, display_name, pfp_url } = profile;
  return { fid, username, display_name, pfp_url };
}

// A count as the card shows it: in full below 10,000, then to three figures
// ("12.3K", "1.2M"), rounded down so it never claims more than it read; a
// count that stopped short (`exact` false) ends in "+".
export function countLabel(n: number, exact = true): string {
  const plus = exact ? "" : "+";
  if (n < 10_000) return n.toLocaleString("en-US") + plus;
  const [value, unit] = n < 1_000_000 ? [n / 1_000, "K"] : [n / 1_000_000, "M"];
  const figures = value < 100 ? Math.floor(value * 10) / 10 : Math.floor(value);
  return `${figures}${unit}${plus}`;
}

// A box on screen, in the pane's coordinates.
export type Rect = { top: number; left: number; bottom: number; right: number };
export type Size = { width: number; height: number };

// Where the card goes: below what it describes, or above when it fits only
// there, or has more room there; its left edge with the anchor's, moved in
// to stay inside the pane. A card larger than the pane starts at its edge.
export function place(anchor: Rect, card: Size, pane: Size): { top: number; left: number; above: boolean } {
  const below = anchor.bottom + GAP;
  const over = anchor.top - GAP - card.height;
  const fitsBelow = below + card.height <= pane.height - MARGIN;
  const fitsAbove = over >= MARGIN;
  const above = !fitsBelow && (fitsAbove || anchor.top > pane.height - anchor.bottom);
  return {
    top: clamp(above ? over : below, MARGIN, pane.height - MARGIN - card.height),
    left: clamp(anchor.left, MARGIN, pane.width - MARGIN - card.width),
    above,
  };
}

const clamp = (v: number, low: number, high: number) => Math.max(low, Math.min(v, high));
