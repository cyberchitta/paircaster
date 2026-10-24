import { render } from "preact";
import { useEffect, useRef, useState } from "preact/hooks";

import { BRANCH_READ, type Branch, branchOf, seedOf } from "./branch";
import type { Tally } from "./card";
import { DEPTH, type FoldKey, NARROW_DEPTH, type Nest } from "./fold";
import { host } from "./host";
import { type Spot, atOf, monikerOf, parseAt } from "./moniker";
import { makeRuntime, type Paircaster } from "./runtime";
import { type Shown, onScreen, reach, read, visible } from "./shown";
import { type Kept, type Live, type Place, isKept, placeOf, reveal, tell } from "./state";
import { type Cast, refOf } from "./thread";
import { ListView } from "./views/list";
import { ThreadView } from "./views/thread";

// The node the CLI reads when --node is not given.
const DEFAULT_NODE = "http://localhost:3381";
// How many casts' reactions and replies are read at once.
const COUNTING = 6;
// A pane at least this wide nests replies DEPTH levels deep, else
// NARROW_DEPTH; viewer.css narrows the nesting at the same width.
const WIDE = "(min-width: 480px)";

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
  // The casts whose counts this view has asked for, and those still to read.
  const asked = useRef(new Set<string>());
  const queue = useRef<string[]>([]);
  // The readers working through `queue` for the load numbered `seq`.
  const workers = useRef({ seq: 0, n: 0 });
  const depth = useDepth();
  const runtime = useRef<Paircaster | null>(null);
  const latest = useRef(0);
  const live = useRef<Live>({
    node: DEFAULT_NODE,
    spot: null,
    shown: null,
    marked: [],
    trail: [],
    opened: new Set(),
    nest: { branches: new Map(), depth: DEPTH },
    userSelected: null,
  });
  const nest: Nest = { branches, depth };
  live.current = { ...live.current, shown: view.shown, marked, trail, opened, nest };

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
      const rt = runtime.current;
      const shown = await read(rt, spot);
      const strays = marks.filter((m) => !visible(shown).some((c) => refOf(c) === m));
      if (strays.length > 0 && shown.kind === "casts") {
        throw new Error(`${strays[0]} is not on the page ${place.at} shows`);
      }
      // A marked reply nested deeper is reached by reading the way down to it.
      const ways =
        shown.kind === "thread"
          ? await Promise.all(strays.map((m) => reach(rt, shown.thread, m, branchRef.current)))
          : [];
      if (seq !== latest.current) return null;
      const none = new Set<string>();
      const now = { node: place.node, spot, shown, marked: marks, trail: place.trail, opened: none };
      live.current = { ...live.current, ...now };
      setView({ shown, error: null, loading: false });
      setTrail(place.trail);
      setMarked(marks);
      setOpened(none);
      setTallies(new Map());
      asked.current = new Set();
      queue.current = [];
      workers.current = { seq, n: 0 };
      const seeds = new Map(branchRef.current);
      ways.forEach((w) => w.forEach((b, ref) => seeds.set(ref, b)));
      if (shown.kind === "thread") {
        const seed = seedOf(shown.thread);
        if (seed) seeds.set(refOf(shown.thread.cast), seed);
      }
      keepBranches(seeds);
      save(live.current);
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
  // Replies read are drawn under their cast and counted in turn, so the tree
  // fills a level at a time, as deep as the page nests and no further.
  const fill = (rt: Paircaster, casts: Cast[], seq: number) => {
    if (workers.current.seq !== seq) return;
    const fresh = casts.map(refOf).filter((r) => !asked.current.has(r));
    fresh.forEach((r) => asked.current.add(r));
    queue.current.push(...fresh);
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
      while (seq === latest.current) {
        const ref = queue.current.shift();
        if (!ref) break;
        await Promise.all([reactions(ref), replies(ref)]);
      }
      if (workers.current.seq === seq) workers.current.n--;
    };
    for (; workers.current.n < COUNTING && queue.current.length > 0; workers.current.n++) worker();
  };

  // Whatever is drawn, at a load, an opened fold or a branch read, is counted.
  useEffect(() => {
    if (!view.shown || !runtime.current) return;
    fill(runtime.current, onScreen(view.shown, opened, marked, nest).drawn, latest.current);
  }, [view.shown, opened, marked, branches, depth]);

  // The owner opens a fold; its casts are then counted like the rest.
  const unfold = (fold: FoldKey) => {
    const next = new Set(opened).add(fold);
    live.current.opened = next;
    setOpened(next);
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
          depth={depth}
          onUnfold={unfold}
          {...actions}
        />
      ) : (
        <ListView casts={shown.casts} accounts={shown.accounts} {...actions} />
      )}
    </main>
  );
}

// How deep the page nests replies, as wide as the pane is now.
function useDepth(): number {
  const [wide, setWide] = useState(() => matchMedia(WIDE).matches);
  useEffect(() => {
    const query = matchMedia(WIDE);
    const change = (e: MediaQueryListEvent) => setWide(e.matches);
    query.addEventListener("change", change);
    return () => query.removeEventListener("change", change);
  }, []);
  return wide ? DEPTH : NARROW_DEPTH;
}

function save(now: Live) {
  if (!now.spot) return;
  const kept: Kept = { node: now.node, at: atOf(project, now.spot), trail: now.trail, user_selected: now.userSelected };
  host?.keep(kept);
}

render(<App />, document.getElementById("app")!);
