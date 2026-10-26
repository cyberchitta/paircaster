import { render } from "preact";
import { useEffect, useRef, useState } from "preact/hooks";

import { type Profiles, profileJob, wanted } from "./author";
import { BRANCH_READ, type Branch, branchOf, seedOf } from "./branch";
import type { Tally } from "./card";
import { type Context, wantedOf } from "./context";
import { DEPTH, type FoldKey, NARROW_DEPTH, type Nest } from "./fold";
import { host } from "./host";
import { Hover } from "./hover";
import { spanned, toggled } from "./marks";
import { atOf, markedOf, monikerOf, parseAt, type Spot } from "./moniker";
import { makeRuntime, type Paircaster } from "./runtime";
import { contextsOf, onScreen, reach, read, type Shown, visible } from "./shown";
import { isKept, type Kept, type Live, type Place, placeOf, reveal, tell } from "./state";
import { type Cast, refOf } from "./thread";
import { ShownView } from "./views/shown";

// The node the CLI reads when --node is not given.
const DEFAULT_NODE = "http://localhost:3381";
// How many casts' reactions and replies, or context casts, are read at once.
const COUNTING = 6;
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

// One read the page makes after it draws: a cast's likes, recasts and
// replies, or a context cast (one it quotes or replies to); or, when the
// owner rests on an author, that account's profile.
type Job = { read: "counts" | "show"; ref: string } | { read: "profile"; fid: number };

const jobKey = (j: Job) => (j.read === "profile" ? profileJob(j.fid) : `${j.read} ${j.ref}`);

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
  // The context casts read with `show`, kept like `branches`; `null` for one
  // the node does not hold.
  const [fetched, setFetched] = useState<Map<string, Context | null>>(new Map());
  const fetchedRef = useRef(fetched);
  // The profiles read for the author card, kept like `branches`; `null` for
  // one the node refused.
  const [profiles, setProfiles] = useState<Profiles>(new Map());
  const profilesRef = useRef(profiles);
  // The jobs this view has asked for (`read ref`), and those still to do.
  const asked = useRef(new Set<string>());
  const queue = useRef<Job[]>([]);
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
    userMarked: null,
  });
  // What the owner marked on this view, and the cast they last pointed at,
  // from which Shift marks a span.
  const userMarks = useRef<string[]>([]);
  const anchor = useRef<string | null>(null);
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
      keepFetched(new Map());
      keepProfiles(new Map());
    }
    setView((v) => ({ ...v, loading: true }));
    try {
      const rt = runtime.current;
      const known = spot.kind === "profile" ? profilesRef.current.get(spot.fid) : undefined;
      const shown = await read(rt, spot, known ?? undefined);
      const strays = marks.filter((m) => !visible(shown).some((c) => refOf(c) === m));
      if (strays.length > 0 && shown.kind !== "thread") {
        throw new Error(`${strays[0]} is not on the page ${place.at} shows`);
      }
      // A marked reply nested deeper is reached by reading the way down to it.
      const ways = shown.kind === "thread" ? await Promise.all(strays.map((m) => reach(rt, shown.thread, m, branchRef.current))) : [];
      if (seq !== latest.current) return null;
      const none = new Set<string>();
      const now = { node: place.node, spot, shown, marked: marks, trail: place.trail, opened: none };
      live.current = { ...live.current, ...now };
      setView({ shown, error: null, loading: false });
      setTrail(place.trail);
      setMarked(marks);
      userMarks.current = [];
      anchor.current = null;
      setOpened(none);
      setTallies(new Map());
      asked.current = new Set();
      queue.current = [];
      workers.current = { seq, n: 0 };
      const seeds = new Map([...branchRef.current, ...ways.flatMap((w) => [...w])]);
      if (shown.kind === "thread") {
        const seed = seedOf(shown.thread);
        if (seed) seeds.set(refOf(shown.thread.cast), seed);
      }
      keepBranches(seeds);
      // The profile read is the author card's too.
      if (shown.kind === "profile") keepProfiles(new Map(profilesRef.current).set(shown.profile.fid, shown.profile));
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

  const keepFetched = (next: Map<string, Context | null>) => {
    fetchedRef.current = next;
    setFetched(next);
  };

  const keepProfiles = (next: Profiles) => {
    profilesRef.current = next;
    setProfiles(next);
  };

  // Fills in each drawn cast's likes, recasts and replies as they arrive, a
  // few casts at a time, after the page is drawn; a later load stops it. A
  // cast's replies are read once per node, its reactions once per view.
  // Replies read are drawn under their cast and counted in turn, so the tree
  // fills a level at a time, as deep as the page nests and no further. The
  // casts they quote, and in a list those they reply to, are read in the
  // same queue, once per node, unless the page holds them already.
  const fill = (rt: Paircaster, shown: Shown, casts: readonly Cast[], seq: number) => {
    if (workers.current.seq !== seq) return;
    const held = contextsOf(shown, nest, fetchedRef.current);
    const jobs: Job[] = casts.flatMap((c) => [
      { read: "counts" as const, ref: refOf(c) },
      ...wantedOf([c], shown.kind !== "thread")
        .filter((r) => !held.has(r))
        .map((ref) => ({ read: "show" as const, ref })),
    ]);
    const fresh = jobs.filter((j) => !asked.current.has(jobKey(j)));
    for (const j of fresh) asked.current.add(jobKey(j));
    queue.current.push(...fresh);
    pump(rt, seq);
  };

  // The owner rests on an author: their profile is read next, ahead of the
  // counts still queued, once per node.
  const want = (fid: number) => {
    const rt = runtime.current;
    if (!rt || !wanted(profilesRef.current, asked.current, fid)) return;
    const job: Job = { read: "profile", fid };
    asked.current.add(jobKey(job));
    queue.current.unshift(job);
    pump(rt, latest.current);
  };

  // Starts readers on the queue, up to COUNTING of them for this load.
  const pump = (rt: Paircaster, seq: number) => {
    if (workers.current.seq !== seq) return;
    const worker = async () => {
      while (seq === latest.current) {
        const job = queue.current.shift();
        if (!job) break;
        await run(rt, seq, job);
      }
      if (workers.current.seq === seq) workers.current.n--;
    };
    for (; workers.current.n < COUNTING && queue.current.length > 0; workers.current.n++) worker();
  };

  const run = (rt: Paircaster, seq: number, job: Job): Promise<unknown> => {
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
    // A cast the node refuses is drawn as gone: deleted, or pruned.
    const show = async (ref: string) => {
      if (fetchedRef.current.has(ref)) return;
      const r = await rt.show(ref).catch(() => null);
      const context = r && { cast: r.cast, account: r.accounts.find((a) => a.fid === r.cast.fid) };
      if (seq === latest.current) keepFetched(new Map(fetchedRef.current).set(ref, context));
    };
    // A profile the node refuses leaves the card with what the page holds.
    const profile = async (fid: number) => {
      if (profilesRef.current.has(fid)) return;
      const r = await rt.profile(fid).catch(() => null);
      if (seq === latest.current) keepProfiles(new Map(profilesRef.current).set(fid, r));
    };
    if (job.read === "profile") return profile(job.fid);
    return job.read === "show" ? show(job.ref) : Promise.all([reactions(job.ref), replies(job.ref)]);
  };

  // Whatever is drawn, at a load, an opened fold or a branch read, is counted.
  useEffect(() => {
    if (!view.shown || !runtime.current) return;
    fill(runtime.current, view.shown, onScreen(view.shown, opened, marked, nest).drawn, latest.current);
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
    const restore = () => {
      if (!isKept(kept)) return Promise.resolve();
      live.current.userSelected = kept.user_selected;
      live.current.userMarked = kept.user_marked ?? null;
      return load(placeOf(kept)).catch(() => {});
    };
    const restored = restore();
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
      if (shown) {
        live.current.userMarked = null;
        point(parseAt(project, at).spot, null);
      }
    } catch (e) {
      setView((v) => ({ ...v, error: String(e instanceof Error ? e.message : e) }));
    }
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
  const contexts = contextsOf(shown, nest, fetched);
  // The agent's line about the view, drawn as text: never as markup.
  const caption = live.current.spot?.caption;
  const actions = { marked, tallies, branches, contexts, onOpen: (ref: string) => go(ref), onSelect: choose, onMark: mark };
  return (
    <Hover profiles={profiles} want={want}>
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
