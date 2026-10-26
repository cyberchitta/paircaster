// The page under a host: what the host kept is shown again, and then the
// host's reveals and tells are answered.

import { useEffect } from "preact/hooks";

import { host } from "./host";
import type { Shown } from "./shown";
import { isKept, type Live, type Place, placeOf, reveal, tell } from "./state";

// Runs once, at the first render. `load` shows a place; `live` is the page's
// state, which a reveal or tell reads when it arrives.
export function useHost(project: string, load: (place: Place) => Promise<Shown | null>, live: { current: Live }) {
  useEffect(() => {
    const connected = host;
    if (!connected) return;
    const kept = connected.kept;
    const restore = () => {
      if (!isKept(kept)) return Promise.resolve();
      live.current.userSelected = kept.user_selected;
      live.current.userMarked = kept.user_marked ?? null;
      return load(placeOf(kept)).catch(() => {});
    };
    restore().then(() =>
      connected.start({
        reveal: (moniker, state) => reveal(project, load, live.current, moniker, state),
        tell: (body) => tell(live.current, body),
      }),
    );
  }, []);
}
