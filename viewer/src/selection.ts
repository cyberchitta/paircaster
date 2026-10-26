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
  // What the owner marked on this view, and the cast they last pointed at,
  // from which Shift marks a span.
  const userMarks = useRef<string[]>([]);
  const anchor = useRef<string | null>(null);

  // A new view: nothing on it is marked or pointed at yet.
  const reset = () => {
    userMarks.current = [];
    anchor.current = null;
  };

  const point = (spot: Spot, chosen: string | null) => {
    const focus = spot.kind === "thread" ? spot.cast : null;
    live.current.userSelected = monikerOf(project, spot) + (chosen && chosen !== focus ? `#cast=${chosen}` : "");
    save(live.current);
  };

  // A click on the focus marks it alone; the keyboard reaching a card
  // selects it the same way, but only while the owner has marked nothing,
  // and is not itself a mark.
  const choose = (cast: Cast, glance = false) => {
    const spot = live.current.spot;
    const ref = refOf(cast);
    if (!spot || (glance && userMarks.current.length > 0)) return;
    anchor.current = ref;
    if (!glance) {
      userMarks.current = [ref];
      live.current.userMarked = markedOf(project, spot, [ref]);
    }
    setMarked([ref]);
    live.current.marked = [ref];
    point(spot, ref);
  };

  // Cmd/Ctrl-click or Space adds a cast to the owner's marks or takes it
  // out; Shift marks every cast drawn from the last one pointed at.
  const mark = (cast: Cast, span: boolean) => {
    const spot = live.current.spot;
    const shown = live.current.shown;
    if (!spot || !shown) return;
    const ref = refOf(cast);
    const order = onScreen(shown, live.current.opened, live.current.marked, live.current.nest).drawn.map(refOf);
    const next = span ? spanned(userMarks.current, order, anchor.current, ref) : toggled(userMarks.current, ref);
    userMarks.current = next;
    anchor.current = ref;
    setMarked(next);
    live.current.marked = next;
    live.current.userMarked = markedOf(project, spot, next);
    if (next.includes(ref)) point(spot, ref);
    else save(live.current);
  };

  return { reset, point, choose, mark };
}
