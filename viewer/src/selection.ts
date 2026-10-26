// What the owner points at and marks: their selection, kept in the page's
// state, which a tell reads under either host. Never what a tell or reveal
// did.

import { useRef } from "preact/hooks";

import { spanned, toggled } from "./marks";
import { markedOf, monikerOf, type Spot } from "./moniker";
import { onScreen } from "./shown";
import type { Live } from "./state";
import { type Cast, refOf } from "./thread";

// `live` is the page's state, `setMarked` draws the marks, and `save` keeps
// the state with the host.
export function useSelection(project: string, live: { current: Live }, setMarked: (marks: string[]) => void, save: (now: Live) => void) {
  const { userMarks, anchor, reset, restore } = useOwnMarks();

  const point = (spot: Spot, chosen: string | null) => {
    live.current.userSelected = selectedOf(project, spot, chosen);
    save(live.current);
  };

  // Draws `next` marked, `ref` being the cast the owner pointed at; `own`
  // records them as the owner's marks too.
  const draw = (spot: Spot, next: string[], ref: string, own: boolean) => {
    anchor.current = ref;
    if (own) {
      userMarks.current = next;
      live.current.userMarked = markedOf(project, spot, next);
    }
    setMarked(next);
    live.current.marked = next;
  };

  // A click on the focus marks it alone; the keyboard reaching a card
  // selects it the same way, but only while the owner has marked nothing,
  // and is not itself a mark.
  const choose = (cast: Cast, glance = false) => {
    const spot = live.current.spot;
    const ref = refOf(cast);
    if (!spot || (glance && userMarks.current.length > 0)) return;
    draw(spot, [ref], ref, !glance);
    point(spot, ref);
  };

  // Cmd/Ctrl-click or Space adds a cast to the owner's marks or takes it
  // out; Shift marks every cast drawn from the last one pointed at.
  const mark = (cast: Cast, span: boolean) => {
    const { spot, shown, opened, marked, nest } = live.current;
    if (!spot || !shown) return;
    const ref = refOf(cast);
    const order = onScreen(shown, opened, marked, nest).drawn.map(refOf);
    const next = span ? spanned(userMarks.current, order, anchor.current, ref) : toggled(userMarks.current, ref);
    draw(spot, next, ref, true);
    if (next.includes(ref)) point(spot, ref);
    else save(live.current);
  };

  return { reset, restore, point, choose, mark };
}

export type Selection = ReturnType<typeof useSelection>;

// What the owner marked on this view, and the cast they last pointed at,
// from which Shift marks a span.
function useOwnMarks() {
  const userMarks = useRef<string[]>([]);
  const anchor = useRef<string | null>(null);

  // A new view: nothing on it is marked or pointed at yet.
  const reset = () => {
    userMarks.current = [];
    anchor.current = null;
  };

  // Marks the owner made before the page reloaded, drawn again: what they
  // mark next adds to them, and Shift spans from the last.
  const restore = (marks: string[]) => {
    userMarks.current = marks;
    anchor.current = marks.at(-1) ?? null;
  };

  return { userMarks, anchor, reset, restore };
}

// The moniker of what the owner chose: the view, and the cast on it unless
// that is the thread's own focus.
function selectedOf(project: string, spot: Spot, chosen: string | null): string {
  const focus = spot.kind === "thread" ? spot.cast : null;
  return monikerOf(project, spot) + (chosen && chosen !== focus ? `#cast=${chosen}` : "");
}
