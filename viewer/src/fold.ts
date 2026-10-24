// Which casts of a thread the page folds into one line ("+8 more"), opened
// by a click. A run holding a marked cast is never folded.

import { type Cast, type Thread, refOf } from "./thread";

// Direct replies shown before the rest fold.
export const FIRST_REPLIES = 10;

// The folds a thread has, by name: what `opened` holds once clicked.
export type FoldKey = "above" | "replies";

export type Piece = { cast: Cast } | { fold: FoldKey; casts: Cast[] };

// The casts above the focus, and its replies, as the page lays them out.
export type Layout = { above: Piece[]; replies: Piece[] };

const each = (casts: Cast[]): Piece[] => casts.map((cast) => ({ cast }));

export function layout(
  thread: Thread,
  only: string[] | null,
  opened: ReadonlySet<string>,
  marked: string[],
): Layout {
  const run = (fold: FoldKey, casts: Cast[]): Piece[] =>
    casts.length === 0 || opened.has(fold) || casts.some((c) => marked.includes(refOf(c)))
      ? each(casts)
      : [{ fold, casts }];
  const { ancestors: a, replies } = thread;
  // The root and the cast replied to stay; a fold of one cast saves nothing.
  const above =
    a.length < 4 ? each(a) : [{ cast: a[0] }, ...run("above", a.slice(1, -1)), { cast: a[a.length - 1] }];
  if (only) {
    const picks = only.map((r) => replies.find((c) => refOf(c) === r)!);
    return { above, replies: [...each(picks), ...run("replies", replies.filter((c) => !only.includes(refOf(c))))] };
  }
  const rest = replies.slice(FIRST_REPLIES);
  return {
    above,
    replies: [...each(replies.slice(0, FIRST_REPLIES)), ...(rest.length < 2 ? each(rest) : run("replies", rest))],
  };
}

const pieces = (l: Layout) => [...l.above, ...l.replies];

// The casts the page holds but shows only as a fold.
export const foldedOf = (l: Layout): Cast[] => pieces(l).flatMap((p) => ("fold" in p ? p.casts : []));

// The casts drawn as cards, in page order; the focus is not in a layout.
export const drawnOf = (l: Layout): Cast[] => pieces(l).flatMap((p) => ("cast" in p ? [p.cast] : []));
