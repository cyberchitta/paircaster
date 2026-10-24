// The casts the owner marks on a view: one by a click, several by
// Cmd/Ctrl-click or Space (each adds or removes one), or a span by
// Shift-click or Shift-Space (every cast drawn between the last one pointed
// at and this one). The marks are named in page order of marking, and never
// more than a moniker can hold.

import { MAX_LISTED } from "./moniker";

// `marks` with `ref` added at the end, or taken out if it is there.
export const toggled = (marks: string[], ref: string): string[] =>
  marks.includes(ref) ? marks.filter((m) => m !== ref) : [...marks, ref].slice(0, MAX_LISTED);

// `marks` with every cast of `order` (the page's, top to bottom) from
// `anchor` to `ref` added, in page order; just `ref` when the anchor is not
// on the page. Nothing past MAX_LISTED is added.
export function spanned(marks: string[], order: string[], anchor: string | null, ref: string): string[] {
  const from = anchor === null ? -1 : order.indexOf(anchor);
  const to = order.indexOf(ref);
  const span = from < 0 || to < 0 ? [ref] : order.slice(Math.min(from, to), Math.max(from, to) + 1);
  const added = span.filter((r) => !marks.includes(r));
  return [...marks, ...added].slice(0, MAX_LISTED);
}
