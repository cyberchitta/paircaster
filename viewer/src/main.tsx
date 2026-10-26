import { render } from "preact";
import { useEffect, useRef, useState } from "preact/hooks";

import { seedOf } from "./branch";
import { useHost } from "./connect";
import { DEPTH, type FoldKey, NARROW_DEPTH, type Nest } from "./fold";
import { host } from "./host";
import { Hover } from "./hover";
import { atOf, parseAt } from "./moniker";
import { useQueue } from "./queue";
import { makeRuntime, type Paircaster } from "./runtime";
import { useSelection } from "./selection";
import { contextsOf, onScreen, reach, read, type Shown, visible } from "./shown";
import type { Kept, Live, Place } from "./state";
import { refOf } from "./thread";
import { ShownView } from "./views/shown";

// The node the CLI reads when --node is not given.
const DEFAULT_NODE = "http://localhost:3381";
// A pane at least this wide nests replies DEPTH levels deep, else
// NARROW_DEPTH; viewer.css narrows the nesting at the same width.
const WIDE = "(min-width: 480px)";

// The project whose monikers this page takes; a page with no host takes none.
const project = host?.project ?? "";

interface View {
  shown: Shown | null;
  error: string | null;
  loading: boolean;
}

function App() {
  const [view, setView] = useState<View>({ shown: null, error: null, loading: false });
  const [trail, setTrail] = useState<string[]>([]);
  const [marked, setMarked] = useState<string[]>([]);
  const [opened, setOpened] = useState<ReadonlySet<string>>(new Set());
  const depth = useDepth();
  const runtime = useRef<Paircaster | null>(null);
  const latest = useRef(0);
  // The reads after a draw, and what they read, kept across views of one node.
  const reads = useQueue(runtime, latest);
  const live = useRef<Live>({
    node: DEFAULT_NODE,
    spot: null,
    shown: null,
    marked: [],
    trail: [],
    opened: new Set(),
    nest: { branches: new Map(), depth: DEPTH },
    userSelected: null,
    userMarked: null,
  });
  const selection = useSelection(project, live, setMarked, save);
  const nest: Nest = { branches: reads.branches, depth };
  live.current = { ...live.current, shown: view.shown, marked, trail, opened, nest };

  // Shows `at` with `marks` marked, or throws and changes nothing; a load
  // overtaken by a later one changes nothing and returns null.
  const load = async (place: Place): Promise<Shown | null> => {
    const { spot, marks } = parseAt(project, place.at);
    const seq = ++latest.current;
    if (runtime.current?.node !== place.node) {
      runtime.current = makeRuntime(place.node);
      reads.forget();
    }
    setView((v) => ({ ...v, loading: true }));
    try {
      const rt = runtime.current;
      const known = spot.kind === "profile" ? reads.held.profiles.current.get(spot.fid) : undefined;
      const shown = await read(rt, spot, known ?? undefined);
      const strays = marks.filter((m) => !visible(shown).some((c) => refOf(c) === m));
      if (strays.length > 0 && shown.kind !== "thread") {
        throw new Error(`${strays[0]} is not on the page ${place.at} shows`);
      }
      // A marked reply nested deeper is reached by reading the way down to it.
      const ways = shown.kind === "thread" ? await Promise.all(strays.map((m) => reach(rt, shown.thread, m, reads.held.branches.current))) : [];
      if (seq !== latest.current) return null;
      const none = new Set<string>();
      const now = { node: place.node, spot, shown, marked: marks, trail: place.trail, opened: none };
      live.current = { ...live.current, ...now };
      setView({ shown, error: null, loading: false });
      setTrail(place.trail);
      setMarked(marks);
      selection.reset();
      setOpened(none);
      reads.restart(seq);
      const seeds = new Map([...reads.held.branches.current, ...ways.flatMap((w) => [...w])]);
      if (shown.kind === "thread") {
        const seed = seedOf(shown.thread);
        if (seed) seeds.set(refOf(shown.thread.cast), seed);
      }
      reads.keepBranches(seeds);
      // The profile read is the author card's too.
      if (shown.kind === "profile") reads.keepProfile(shown.profile);
      save(live.current);
      return shown;
    } catch (e) {
      if (seq === latest.current) setView((v) => ({ ...v, loading: false }));
      throw e;
    }
  };

  // Whatever is drawn, at a load, an opened fold or a branch read, is counted.
  useEffect(() => {
    if (!view.shown || !runtime.current) return;
    reads.fill(runtime.current, view.shown, nest, onScreen(view.shown, opened, marked, nest).drawn, latest.current);
  }, [view.shown, opened, marked, reads.branches, depth]);

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

  useHost(project, load, live);

  const go = async (at: string, back = false) => {
    const from = live.current.spot;
    if (!from) return;
    const next = back ? trail.slice(0, -1) : [...trail, atOf(project, from)];
    try {
      const shown = await load({ node: live.current.node, at, trail: next });
      if (shown) {
        live.current.userMarked = null;
        selection.point(parseAt(project, at).spot, null);
      }
    } catch (e) {
      setView((v) => ({ ...v, error: String(e instanceof Error ? e.message : e) }));
    }
  };

  if (!host)
    return (
      <p class="note">
        Open this page with <code>paircaster viewer open</code>.
      </p>
    );
  const shown = view.shown;
  if (!shown) {
    if (view.error || view.loading) return <p class="note">{view.error ?? "Reading the node…"}</p>;
    return (
      <p class="note">
        <strong>Nothing open yet.</strong> Ask your agent to show you a conversation, for example “show me the replies to dwr's latest cast”. Click any cast and
        your agent can see which one you mean.
      </p>
    );
  }
  const contexts = contextsOf(shown, nest, reads.fetched);
  // The agent's line about the view, drawn as text: never as markup.
  const caption = live.current.spot?.caption;
  const actions = {
    marked,
    tallies: reads.tallies,
    branches: reads.branches,
    contexts,
    onOpen: (ref: string) => go(ref),
    onSelect: selection.choose,
    onMark: selection.mark,
  };
  return (
    <Hover profiles={reads.profiles} want={reads.want}>
      <main class={view.loading ? "loading" : ""}>
        {trail.length > 0 && (
          <button type="button" class="back" onClick={() => go(trail[trail.length - 1], true)}>
            ← Back
          </button>
        )}
        {view.error && <p class="error">{view.error}</p>}
        {caption !== undefined && <p class="caption">{caption}</p>}
        <ShownView shown={shown} opened={opened} depth={depth} onUnfold={unfold} {...actions} />
      </main>
    </Hover>
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
  const kept: Kept = {
    node: now.node,
    at: atOf(project, now.spot),
    trail: now.trail,
    user_selected: now.userSelected,
    user_marked: now.userMarked,
  };
  host?.keep(kept);
}

const root = document.getElementById("app");
if (!root) throw new Error("the page has no #app element to draw into");
render(<App />, root);
