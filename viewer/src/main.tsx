import { render } from "preact";
import { useEffect, useRef, useState } from "preact/hooks";

import { BRANCH_READ, type Branch, branchOf, seedOf } from "./branch";
import type { Tally } from "./card";
import type { FoldKey } from "./fold";
import { host } from "./host";
import { type Spot, atOf, monikerOf, parseAt } from "./moniker";
import { makeRuntime, type Paircaster } from "./runtime";
import { type Shown, onScreen, read, visible } from "./shown";
import { type Kept, type Live, type Place, isKept, placeOf, reveal, tell } from "./state";
import { type Cast, refOf } from "./thread";
import { ListView } from "./views/list";
import { ThreadView } from "./views/thread";

// The node the CLI reads when --node is not given.
const DEFAULT_NODE = "http://localhost:3381";
// How many casts' reactions and replies are read at once.
const COUNTING = 6;

// The project whose monikers this page takes; a page with no host takes none.
const project = host?.project ?? "";

type View = { shown: Shown | null; error: string | null; loading: boolean };

function App() {
  const [view, setView] = useState<View>({ shown: null, error: null, loading: false });
  const [trail, setTrail] = useState<string[]>([]);
  const [marked, setMarked] = useState<string[]>([]);
  const [tallies, setTallies] = useState<Map<string, Tally>>(new Map());
  const [opened, setOpened] = useState<ReadonlySet<string>>(new Set());
  // Each cast's direct replies, kept across views of one node: the count on
  // its card, and what opens under it.
  const [branches, setBranches] = useState<Map<string, Branch>>(new Map());
  const branchRef = useRef(branches);
  // The casts whose counts this view has asked for.
  const asked = useRef(new Set<string>());
  const runtime = useRef<Paircaster | null>(null);
  const latest = useRef(0);
  const live = useRef<Live>({
    node: DEFAULT_NODE,
    spot: null,
    shown: null,
    marked: [],
    trail: [],
    opened: new Set(),
    userSelected: null,
  });
  live.current = { ...live.current, shown: view.shown, marked, trail, opened };

  // Shows `at` with `marks` marked, or throws and changes nothing; a load
  // overtaken by a later one changes nothing and returns null.
  const load = async (place: Place): Promise<Shown | null> => {
    const { spot, marks } = parseAt(project, place.at);
    const seq = ++latest.current;
    if (runtime.current?.node !== place.node) {
      runtime.current = makeRuntime(place.node);
      keepBranches(new Map());
    }
    setView((v) => ({ ...v, loading: true }));
    try {
      const shown = await read(runtime.current, spot);
      const stray = marks.find((m) => !visible(shown).some((c) => refOf(c) === m));
      if (stray) throw new Error(`${stray} is not on the page ${place.at} shows`);
      if (seq !== latest.current) return null;
      const none = new Set<string>();
      live.current = { ...live.current, node: place.node, spot, shown, marked: marks, trail: place.trail, opened: none };
      setView({ shown, error: null, loading: false });
      setTrail(place.trail);
      setMarked(marks);
      setOpened(none);
      setTallies(new Map());
      asked.current = new Set();
      if (shown.kind === "thread") {
        const seed = seedOf(shown.thread);
        if (seed) keepBranches(new Map(branchRef.current).set(refOf(shown.thread.cast), seed));
      }
      save(live.current);
      fill(runtime.current, onScreen(shown, none, marks).drawn, seq);
      return shown;
    } catch (e) {
      if (seq === latest.current) setView((v) => ({ ...v, loading: false }));
      throw e;
    }
  };

  const keepBranches = (next: Map<string, Branch>) => {
    branchRef.current = next;
    setBranches(next);
  };

  // Fills in each drawn cast's likes, recasts and replies as they arrive, a
  // few casts at a time, after the page is drawn; a later load stops it. A
  // cast's replies are read once per node, its reactions once per view.
  const fill = (rt: Paircaster, casts: Cast[], seq: number) => {
    const queue = casts.map(refOf).filter((r) => !asked.current.has(r));
    queue.forEach((r) => asked.current.add(r));
    const replies = async (ref: string) => {
      if (branchRef.current.has(ref)) return;
      const r = await rt.replies(ref, { limit: BRANCH_READ }).catch(() => null);
      if (r && seq === latest.current) keepBranches(new Map(branchRef.current).set(ref, branchOf(r)));
    };
    const reactions = async (ref: string) => {
      const r = await rt.reactions(ref).catch(() => null);
      const tally = r && { likes: r.likes.length, recasts: r.recasts.length };
      if (tally && seq === latest.current) setTallies((t) => new Map(t).set(ref, tally));
    };
    const worker = async () => {
      for (let ref = queue.shift(); ref && seq === latest.current; ref = queue.shift()) {
        await Promise.all([reactions(ref), replies(ref)]);
      }
    };
    Array.from({ length: COUNTING }, worker);
  };

  // The owner opens a fold; its casts are then counted like the rest.
  const unfold = (fold: FoldKey) => {
    const shown = live.current.shown;
    if (!shown || !runtime.current) return;
    const next = new Set(opened).add(fold);
    live.current.opened = next;
    setOpened(next);
    fill(runtime.current, onScreen(shown, next, marked).drawn, latest.current);
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
  const actions = { marked, tallies, branches, onOpen: (ref: string) => go(ref), onSelect: choose };
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
          opened={opened}
          onUnfold={unfold}
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
