// The page under a host: what the host kept is shown again, and then the
// host's reveals and tells are answered.

import { useEffect } from "preact/hooks";

import { host } from "./host";
import { parseAt } from "./moniker";
import type { Shown } from "./shown";
import { isKept, type Live, type Place, placeOf, reveal, tell } from "./state";

// Runs once, at the first render. `load` shows a place; `live` is the page's
// state, which a reveal or tell reads when it arrives; `remark` hands the
// owner's restored marks back to the selection.
export function useHost(project: string, load: (place: Place) => Promise<Shown | null>, live: { current: Live }, remark: (marks: string[]) => void) {
  useEffect(() => {
    const connected = host;
    if (!connected) return;
    const kept = connected.kept;
    // The view the owner was on, with what they marked on it drawn marked.
    // Where the marked view no longer shows (a cast deleted since), the view
    // alone, and nothing marked.
    const restore = async () => {
      if (!isKept(kept)) return;
      // Set before a load, which keeps them with the host as it finishes.
      live.current.userSelected = kept.user_selected;
      live.current.userMarked = kept.user_marked ?? null;
      const place = placeOf(kept);
      const marked = live.current.userMarked;
      if (marked !== null && (await load({ ...place, at: marked }).catch(() => null))) {
        remark(parseAt(project, marked).marks);
        return;
      }
      live.current.userMarked = null;
      await load(place).catch(() => {});
    };
    restore().then(() =>
      connected.start({
        reveal: (moniker, state) => reveal(project, load, live.current, moniker, state),
        tell: (body) => tell(live.current, body),
      }),
    );
  }, []);
}
