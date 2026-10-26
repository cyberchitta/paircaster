// How the page lays out a thread: the casts above the focus, and its replies
// nested under one another from the replies the page has read, with long
// runs folded into one line ("+8 more") opened by a click. A run holding a
// marked cast, or a cast a marked one is nested under, never folds, and
// the way down to a marked cast is drawn however deep it is.

import type { Branch } from "./branch";
import { type Cast, type Thread, refOf } from "./thread";

// Direct replies shown before the rest fold.
export const FIRST_REPLIES = 10;
// Replies shown at each level, the focus's own replies first; deeper levels
// show as many as the last.
export const SHOWN = [FIRST_REPLIES, 3, 1];
// How many levels of replies the page nests, and in a narrow pane; a reply
// at the last level with replies of its own continues in its own thread.
export const DEPTH = 3;
export const NARROW_DEPTH = 2;

// A fold by name, what `opened` holds once clicked: `above`, `replies` (the
// focus's), or `replies:<FID:HASH>` (a nested reply's).
export type FoldKey = string;
export const repliesFold = (ref: string): FoldKey => `replies:${ref}`;

// What the page has read below the focus, and how deep it nests.
export type Nest = { branches: ReadonlyMap<string, Branch>; depth: number };

// A cast drawn as a card, with its replies laid out under it, or `continues`
// when it has replies past the depth the page nests; or a folded run.
export type Piece = { cast: Cast; replies: Piece[]; continues: boolean } | { fold: FoldKey; casts: Cast[] };

// The casts above the focus, and its replies, as the page lays them out.
export type Layout = { above: Piece[]; replies: Piece[] };

const leaf = (cast: Cast): Piece => ({ cast, replies: [], continues: false });

// The marked casts and every cast they are nested under, up to the focus.
function leadsOf(thread: Thread, marked: string[], nest: Nest): Set<string> {
  const focus = refOf(thread.cast);
  const parent = new Map(thread.replies.map((c) => [refOf(c), focus]));
  nest.branches.forEach((b, ref) => b.casts.forEach((c) => parent.set(refOf(c), ref)));
  const leads = new Set<string>();
  for (const m of marked) {
    for (let r: string | undefined = m; r && r !== focus && !leads.has(r); r = parent.get(r)) leads.add(r);
  }
  return leads;
}

export function layout(thread: Thread, only: string[] | null, opened: ReadonlySet<string>, marked: string[], nest: Nest): Layout {
  const leads = leadsOf(thread, marked, nest);
  const run = (fold: FoldKey, casts: Cast[], each: (c: Cast) => Piece): Piece[] =>
    casts.length === 0 || opened.has(fold) || casts.some((c) => leads.has(refOf(c))) ? casts.map(each) : [{ fold, casts }];
  const node =
    (depth: number) =>
    (cast: Cast): Piece => {
      const below = nest.branches.get(refOf(cast))?.casts ?? [];
      if (below.length === 0) return leaf(cast);
      if (depth >= nest.depth && !below.some((c) => leads.has(refOf(c)))) return { ...leaf(cast), continues: true };
      return { cast, replies: level(repliesFold(refOf(cast)), below, depth + 1), continues: false };
    };
  // A level's first few replies, and the rest folded when two or more remain.
  const level = (fold: FoldKey, casts: Cast[], depth: number): Piece[] => {
    const shown = SHOWN[Math.min(depth, SHOWN.length) - 1];
    const rest = casts.slice(shown);
    const each = node(depth);
    return [...casts.slice(0, shown).map(each), ...(rest.length < 2 ? rest.map(each) : run(fold, rest, each))];
  };
  const { ancestors: a, replies } = thread;
  // The root and the cast replied to stay; a fold of one cast saves nothing.
  const above = a.length < 4 ? a.map(leaf) : [leaf(a[0]), ...run("above", a.slice(1, -1), leaf), leaf(a[a.length - 1])];
  if (only) {
    const picks = only.map((r) => replies.find((c) => refOf(c) === r)!);
    const rest = replies.filter((c) => !only.includes(refOf(c)));
    return { above, replies: [...picks.map(node(1)), ...run("replies", rest, node(1))] };
  }
  return { above, replies: level("replies", replies, 1) };
}

// Every piece of a layout, nested ones after the card they are under.
const flat = (ps: Piece[]): Piece[] => ps.flatMap((p) => ("cast" in p ? [p, ...flat(p.replies)] : [p]));
const pieces = (l: Layout) => flat([...l.above, ...l.replies]);

// The casts the page holds but shows only as a fold.
export const foldedOf = (l: Layout): Cast[] => pieces(l).flatMap((p) => ("fold" in p ? p.casts : []));

// The casts drawn as cards, in page order; the focus is not in a layout.
export const drawnOf = (l: Layout): Cast[] => pieces(l).flatMap((p) => ("cast" in p ? [p.cast] : []));

// The cards whose replies continue in their own thread.
export const continuedOf = (l: Layout): Cast[] => pieces(l).flatMap((p) => ("cast" in p && p.continues ? [p.cast] : []));
