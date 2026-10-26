import { render } from "preact";
import { type Dispatch, type StateUpdater, useEffect, useRef, useState } from "preact/hooks";

import { type Branch, seedOf } from "./branch";
import { useHost } from "./connect";
import { DEPTH, type FoldKey, NARROW_DEPTH, type Nest } from "./fold";
import { host } from "./host";
import { Hover } from "./hover";
import { atOf, parseAt, type Spot } from "./moniker";
import { type Queue, useQueue } from "./queue";
import { makeRuntime, type Paircaster } from "./runtime";
import { type Selection, useSelection } from "./selection";
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

// What the page draws: the view read, or the error or wait in its place; the
// way back; the casts marked; the folds the owner opened.
interface Page {
  shown: Shown | null;
  error: string | null;
  loading: boolean;
  trail: string[];
  marked: string[];
  opened: ReadonlySet<string>;
}

const BLANK: Page = { shown: null, error: null, loading: false, trail: [], marked: [], opened: new Set() };

function App() {
  const depth = useDepth();
  const [page, setPage] = useState<Page>(BLANK);
  const runtime = useRef<Paircaster | null>(null);
  const latest = useRef(0);
  // The reads after a draw, and what they read, kept across views of one node.
  const reads = useQueue(runtime, latest);
  const live = useRef<Live>(startOf());
  const selection = useSelection(project, live, (marked) => setPage((p) => ({ ...p, marked })), save);
  const { shown, marked, trail, opened } = page;
  const nest: Nest = { branches: reads.branches, depth };
  live.current = { ...live.current, shown, marked, trail, opened, nest };
  const pager: Pager = { runtime, latest, live, reads, selection, setPage };
  // Whatever is drawn, at a load, an opened fold or a branch read, is counted.
  useEffect(() => {
    if (!shown || !runtime.current) return;
    reads.fill(runtime.current, shown, nest, onScreen(shown, opened, marked, nest).drawn, latest.current);
  }, [shown, opened, marked, reads.branches, depth]);
  useOpenAtMark(live, shown);
  useHost(project, (place) => loadWith(pager, place), live, selection.restore);
  if (!shown) return <Empty error={page.error} loading={page.loading} />;
  const unfold = (fold: FoldKey) => unfoldWith(pager, opened, fold);
  const go = (at: string, back = false) => goWith(pager, trail, at, back);
  return <Drawn page={page} shown={shown} reads={reads} nest={nest} caption={live.current.spot?.caption} selection={selection} go={go} unfold={unfold} />;
}

// What a load and a move between views work on. All of it outlives a render,
// so a load the host began on the first render still reaches the page.
interface Pager {
  runtime: { current: Paircaster | null };
  latest: { current: number };
  live: { current: Live };
  reads: Pick<Queue, "forget" | "held" | "restart" | "keepBranches" | "keepProfile">;
  selection: Pick<Selection, "reset" | "point">;
  setPage: Dispatch<StateUpdater<Page>>;
}

// Shows `at` with `marks` marked, or throws and changes nothing; a load
// overtaken by a later one changes nothing and returns null.
async function loadWith(p: Pager, place: Place): Promise<Shown | null> {
  const { spot, marks } = parseAt(project, place.at);
  const seq = ++p.latest.current;
  if (p.runtime.current?.node !== place.node) {
    p.runtime.current = makeRuntime(place.node);
    p.reads.forget();
  }
  p.setPage((v) => ({ ...v, loading: true }));
  try {
    const { shown, ways } = await readAt(p.runtime.current, spot, marks, place.at, p.reads.held);
    if (seq !== p.latest.current) return null;
    draw(p, place, { spot, marks, shown, ways }, seq);
    return shown;
  } catch (e) {
    if (seq === p.latest.current) p.setPage((v) => ({ ...v, loading: false }));
    throw e;
  }
}

// What `at` shows, and the way down to each marked reply nested deeper than
// the thread read reaches; throws for a mark on neither.
async function readAt(rt: Paircaster, spot: Spot, marks: string[], at: string, held: Queue["held"]) {
  const known = spot.kind === "profile" ? held.profiles.current.get(spot.fid) : undefined;
  const shown = await read(rt, spot, known ?? undefined);
  const strays = marks.filter((m) => !visible(shown).some((c) => refOf(c) === m));
  if (strays.length > 0 && shown.kind !== "thread") {
    throw new Error(`${strays[0]} is not on the page ${at} shows`);
  }
  const ways = shown.kind === "thread" ? await Promise.all(strays.map((m) => reach(rt, shown.thread, m, held.branches.current))) : [];
  return { shown, ways };
}

interface Read {
  spot: Spot;
  marks: string[];
  shown: Shown;
  ways: readonly Map<string, Branch>[];
}

// Draws what a load read: nothing yet marked by the owner or opened, and its
// counts read afresh.
function draw(p: Pager, place: Place, { spot, marks, shown, ways }: Read, seq: number) {
  const none = new Set<string>();
  p.live.current = { ...p.live.current, node: place.node, spot, shown, marked: marks, trail: place.trail, opened: none };
  p.setPage({ shown, error: null, loading: false, trail: place.trail, marked: marks, opened: none });
  p.selection.reset();
  p.reads.restart(seq);
  p.reads.keepBranches(seedsOf(p.reads.held.branches.current, ways, shown));
  // The profile read is the author card's too.
  if (shown.kind === "profile") p.reads.keepProfile(shown.profile);
  save(p.live.current);
}

// The replies a load holds: those kept for the node, those read on the way
// to a mark, and the focus's own where the thread read holds them all.
function seedsOf(kept: ReadonlyMap<string, Branch>, ways: readonly Map<string, Branch>[], shown: Shown): Map<string, Branch> {
  const seeds = new Map([...kept, ...ways.flatMap((w) => [...w])]);
  if (shown.kind !== "thread") return seeds;
  const seed = seedOf(shown.thread);
  if (seed) seeds.set(refOf(shown.thread.cast), seed);
  return seeds;
}

// Opens `at` from the view the owner is on, or goes back to it from there.
async function goWith(p: Pager, trail: string[], at: string, back: boolean) {
  const from = p.live.current.spot;
  if (!from) return;
  const next = back ? trail.slice(0, -1) : [...trail, atOf(project, from)];
  try {
    const shown = await loadWith(p, { node: p.live.current.node, at, trail: next });
    if (!shown) return;
    p.live.current.userMarked = null;
    p.selection.point(parseAt(project, at).spot, null);
  } catch (e) {
    p.setPage((v) => ({ ...v, error: String(e instanceof Error ? e.message : e) }));
  }
}

// The owner opens a fold; its casts are then counted like the rest.
function unfoldWith(p: Pick<Pager, "live" | "setPage">, opened: ReadonlySet<string>, fold: FoldKey) {
  const next = new Set(opened).add(fold);
  p.live.current.opened = next;
  p.setPage((v) => ({ ...v, opened: next }));
}

// The page's state before its first load.
const startOf = (): Live => ({
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

// A new page opens at the first marked cast, else at the top.
function useOpenAtMark(live: { readonly current: Live }, shown: Shown | null) {
  useEffect(() => {
    const first = live.current.marked[0];
    const card = first ? document.querySelector(`[data-cast="${first}"]`) : null;
    card ? card.scrollIntoView({ block: "center" }) : window.scrollTo(0, 0);
  }, [shown]);
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

interface DrawnProps {
  page: Pick<Page, "loading" | "error" | "trail" | "marked" | "opened">;
  shown: Shown;
  reads: Pick<Queue, "tallies" | "branches" | "fetched" | "profiles" | "want">;
  nest: Nest;
  caption: string | undefined;
  selection: Pick<Selection, "choose" | "mark">;
  go(at: string, back?: boolean): void;
  unfold(fold: FoldKey): void;
}

// The view, under the way back, any error, and the agent's caption.
function Drawn({ page, shown, reads, nest, caption, selection, go, unfold }: DrawnProps) {
  const { trail, marked, opened } = page;
  const actions = {
    marked,
    tallies: reads.tallies,
    branches: reads.branches,
    contexts: contextsOf(shown, nest, reads.fetched),
    onOpen: (ref: string) => go(ref),
    onSelect: selection.choose,
    onMark: selection.mark,
  };
  return (
    <Hover profiles={reads.profiles} want={reads.want}>
      <main class={page.loading ? "loading" : ""}>
        {trail.length > 0 && (
          <button type="button" class="back" onClick={() => go(trail[trail.length - 1], true)}>
            ← Back
          </button>
        )}
        {page.error && <p class="error">{page.error}</p>}
        {/* The agent's line about the view, drawn as text: never as markup. */}
        {caption !== undefined && <p class="caption">{caption}</p>}
        <ShownView shown={shown} opened={opened} depth={nest.depth} onUnfold={unfold} {...actions} />
      </main>
    </Hover>
  );
}

function Empty({ error, loading }: Pick<Page, "error" | "loading">) {
  if (error || loading) return <p class="note">{error ?? "Reading the node…"}</p>;
  return (
    <p class="note">
      <strong>Nothing open yet.</strong> Ask your agent to show you a conversation, for example “show me the replies to dwr's latest cast”. Click any cast and
      your agent can see which one you mean.
    </p>
  );
}

const NoHost = () => (
  <p class="note">
    Open this page with <code>paircaster viewer open</code>.
  </p>
);

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
render(host ? <App /> : <NoHost />, root);
