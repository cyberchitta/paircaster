import { render } from "preact";
import { useEffect, useRef, useState } from "preact/hooks";

import type { Tally } from "./card";
import { host } from "./host";
import { type Spot, atOf, monikerOf, parseAt } from "./moniker";
import { makeRuntime, type Paircaster } from "./runtime";
import { type Shown, read, visible } from "./shown";
import { type Kept, type Live, type Place, isKept, placeOf, reveal, tell } from "./state";
import { type Cast, refOf } from "./thread";
import { ListView } from "./views/list";
import { ThreadView } from "./views/thread";

// The node the CLI reads when --node is not given.
const DEFAULT_NODE = "http://localhost:3381";
// How many casts' reactions are read at once.
const COUNTING = 6;

// The project whose monikers this page takes; a page with no host takes none.
const project = host?.project ?? "";

type View = { shown: Shown | null; error: string | null; loading: boolean };

function App() {
  const [view, setView] = useState<View>({ shown: null, error: null, loading: false });
  const [trail, setTrail] = useState<string[]>([]);
  const [marked, setMarked] = useState<string[]>([]);
  const [tallies, setTallies] = useState<Map<string, Tally>>(new Map());
  const runtime = useRef<Paircaster | null>(null);
  const latest = useRef(0);
  const live = useRef<Live>({ node: DEFAULT_NODE, spot: null, shown: null, marked: [], trail: [], userSelected: null });
  live.current = { ...live.current, shown: view.shown, marked, trail };

  // Shows `at` with `marks` marked, or throws and changes nothing; a load
  // overtaken by a later one changes nothing and returns null.
  const load = async (place: Place): Promise<Shown | null> => {
    const { spot, marks } = parseAt(project, place.at);
    const seq = ++latest.current;
    if (runtime.current?.node !== place.node) runtime.current = makeRuntime(place.node);
    setView((v) => ({ ...v, loading: true }));
    try {
      const shown = await read(runtime.current, spot);
      const stray = marks.find((m) => !visible(shown).some((c) => refOf(c) === m));
      if (stray) throw new Error(`${stray} is not on the page ${place.at} shows`);
      if (seq !== latest.current) return null;
      live.current = { ...live.current, node: place.node, spot, shown, marked: marks, trail: place.trail };
      setView({ shown, error: null, loading: false });
      setTrail(place.trail);
      setMarked(marks);
      setTallies(new Map());
      save(live.current);
      countReactions(runtime.current, visible(shown), seq);
      return shown;
    } catch (e) {
      if (seq === latest.current) setView((v) => ({ ...v, loading: false }));
      throw e;
    }
  };

  // Fills in each shown cast's likes and recasts as they arrive, a few at a
  // time, after the page is drawn; a later load stops it.
  const countReactions = (rt: Paircaster, casts: Cast[], seq: number) => {
    const queue = casts.map(refOf);
    const worker = async () => {
      for (let ref = queue.shift(); ref && seq === latest.current; ref = queue.shift()) {
        const r = await rt.reactions(ref).catch(() => null);
        const tally = r && { likes: r.likes.length, recasts: r.recasts.length };
        if (tally && seq === latest.current) setTallies((t) => new Map(t).set(ref!, tally));
      }
    };
    Array.from({ length: COUNTING }, worker);
  };

  // A new page opens at the first marked cast, else at the top.
  useEffect(() => {
    const first = live.current.marked[0];
    const card = first ? document.querySelector(`[data-cast="${first}"]`) : null;
    card ? card.scrollIntoView({ block: "center" }) : window.scrollTo(0, 0);
  }, [view.shown]);

  useEffect(() => {
    const connected = host;
    if (!connected) return;
    const kept = connected.kept;
    const restored = isKept(kept)
      ? ((live.current.userSelected = kept.user_selected), load(placeOf(kept)).catch(() => {}))
      : Promise.resolve();
    restored.then(() =>
      connected.start({
        reveal: (moniker, state) => reveal(project, load, live.current, moniker, state),
        tell: (body) => tell(live.current, body),
      }),
    );
  }, []);

  // What the owner does becomes their selection, in the page's state, which
  // a tell reads under either host. Never what a tell or reveal did.
  const point = (spot: Spot, chosen: string | null) => {
    const focus = spot.kind === "thread" ? spot.cast : null;
    live.current.userSelected = monikerOf(project, spot) + (chosen && chosen !== focus ? `#cast=${chosen}` : "");
    save(live.current);
  };

  const go = async (at: string, back = false) => {
    const from = live.current.spot;
    if (!from) return;
    const next = back ? trail.slice(0, -1) : [...trail, atOf(project, from)];
    try {
      const shown = await load({ node: live.current.node, at, trail: next });
      if (shown) point(parseAt(project, at).spot, null);
    } catch (e) {
      setView((v) => ({ ...v, error: String(e instanceof Error ? e.message : e) }));
    }
  };

  const choose = (cast: Cast) => {
    const spot = live.current.spot;
    if (!spot || (marked.length === 1 && marked[0] === refOf(cast))) return;
    setMarked([refOf(cast)]);
    live.current.marked = [refOf(cast)];
    point(spot, refOf(cast));
  };

  if (!host) return <p class="note">Open this page with <code>paircaster viewer open</code>.</p>;
  const shown = view.shown;
  if (!shown) {
    if (view.error || view.loading) return <p class="note">{view.error ?? "Reading the node…"}</p>;
    return (
      <p class="note">
        <strong>Nothing open yet.</strong> Ask your agent to show you a conversation, for example “show me the
        replies to dwr's latest cast”. Click any cast and your agent can see which one you mean.
      </p>
    );
  }
  const actions = { marked, tallies, onOpen: (ref: string) => go(ref), onSelect: choose };
  return (
    <main class={view.loading ? "loading" : ""}>
      {trail.length > 0 && (
        <button class="back" onClick={() => go(trail[trail.length - 1], true)}>
          ← Back
        </button>
      )}
      {view.error && <p class="error">{view.error}</p>}
      {shown.kind === "thread" ? (
        <ThreadView
          thread={shown.thread}
          only={shown.only}
          onShowAll={() => go(refOf(shown.thread.cast))}
          {...actions}
        />
      ) : (
        <ListView casts={shown.casts} accounts={shown.accounts} {...actions} />
      )}
    </main>
  );
}

function save(now: Live) {
  if (!now.spot) return;
  const kept: Kept = { node: now.node, at: atOf(project, now.spot), trail: now.trail, user_selected: now.userSelected };
  host?.keep(kept);
}

render(<App />, document.getElementById("app")!);
