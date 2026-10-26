// The reads the page makes after it draws: each drawn cast's likes, recasts
// and replies, the casts it quotes or replies to, and the profile of an
// author the owner rests on. What they read is kept per node; a later load
// stops the reads of an earlier one.

import { useRef, useState } from "preact/hooks";

import { type Profiles, profileJob, wanted } from "./author";
import { BRANCH_READ, type Branch, branchOf } from "./branch";
import type { Tally } from "./card";
import { type Context, type Contexts, wantedOf } from "./context";
import type { Nest } from "./fold";
import type { Profile } from "./reads";
import type { Paircaster } from "./runtime";
import { contextsOf, type Shown } from "./shown";
import { type Cast, refOf } from "./thread";

// How many casts' reactions and replies, or context casts, are read at once.
const COUNTING = 6;

// One read: a cast's likes, recasts and replies, or a context cast (one it
// quotes or replies to); or, when the owner rests on an author, their
// profile.
type Job = { read: "counts" | "show"; ref: string } | { read: "profile"; fid: number };

const jobKey = (j: Job) => (j.read === "profile" ? profileJob(j.fid) : `${j.read} ${j.ref}`);

// `runtime` is the page's reader for the current node, and `latest` the
// number of the latest load: a read answering an older load is dropped.
export function useQueue(runtime: { readonly current: Paircaster | null }, latest: { readonly current: number }) {
  const store = useStore();
  const jobs = useJobs(latest, (rt, job, current) => runJob(rt, job, current, store));

  // Fills in each drawn cast's likes, recasts and replies as they arrive, a
  // few casts at a time, after the page is drawn. A cast's replies are read
  // once per node, its reactions once per view. Replies read are drawn under
  // their cast and counted in turn, so the tree fills a level at a time, as
  // deep as the page nests and no further. The casts they quote, and in a
  // list those they reply to, are read in the same queue, once per node,
  // unless the page holds them already.
  const fill = (rt: Paircaster, shown: Shown, nest: Nest, casts: readonly Cast[], seq: number) => {
    if (!jobs.serving(seq)) return;
    jobs.add(rt, seq, jobsOf(casts, shown.kind !== "thread", contextsOf(shown, nest, store.fetched.ref.current)));
  };

  // The owner rests on an author: their profile is read next, ahead of the
  // counts still queued, once per node.
  const want = (fid: number) => {
    const rt = runtime.current;
    if (!rt || !wanted(store.profiles.ref.current, jobs.asked(), fid)) return;
    jobs.add(rt, latest.current, [{ read: "profile", fid }], true);
  };

  // A new load, numbered `seq`: its counts start empty and its queue afresh.
  const restart = (seq: number) => {
    store.setTallies(new Map());
    jobs.restart(seq);
  };

  return { ...store.view(), restart, fill, want };
}

export type Queue = ReturnType<typeof useQueue>;

// What the reads keep. `tallies` are this view's; the rest is kept per node.
function useStore() {
  const [tallies, setTallies] = useState<Map<string, Tally>>(new Map());
  // Each cast's direct replies: the count on its card, and what opens under it.
  const branches = useKept<Map<string, Branch>>(new Map());
  // The context casts read with `show`; `null` for one the node does not hold.
  const fetched = useKept<Map<string, Context | null>>(new Map());
  // The profiles read for the author card; `null` for one the node refused.
  const profiles = useKept<Profiles>(new Map());
  // What the page sees of it.
  const view = () => ({
    tallies,
    branches: branches.value,
    fetched: fetched.value,
    profiles: profiles.value,
    // What is kept, read outside a render.
    held: { branches: branches.ref, profiles: profiles.ref },
    keepBranches: branches.keep,
    // The profile a load read is the author card's too.
    keepProfile: (p: Profile) => profiles.keep(new Map(profiles.ref.current).set(p.fid, p)),
    // A new node: nothing read from the last one is kept.
    forget: () => {
      branches.keep(new Map());
      fetched.keep(new Map());
      profiles.keep(new Map());
    },
  });
  return { setTallies, branches, fetched, profiles, view };
}

type Store = ReturnType<typeof useStore>;

// The jobs a load has asked for, those still to do, and up to COUNTING
// readers working through them for the load numbered `seq`. `run` does one
// job; `current` says whether its load is still the latest.
function useJobs(latest: { readonly current: number }, run: (rt: Paircaster, job: Job, current: () => boolean) => Promise<unknown>) {
  const asked = useRef(new Set<string>());
  const queue = useRef<Job[]>([]);
  const workers = useRef({ seq: 0, n: 0 });

  // Starts readers on the queue, up to COUNTING of them for this load.
  const pump = (rt: Paircaster, seq: number) => {
    if (workers.current.seq !== seq) return;
    const worker = async () => {
      while (seq === latest.current) {
        const job = queue.current.shift();
        if (!job) break;
        await run(rt, job, () => seq === latest.current);
      }
      if (workers.current.seq === seq) workers.current.n--;
    };
    for (; workers.current.n < COUNTING && queue.current.length > 0; workers.current.n++) worker();
  };

  // Queues the jobs this load has not asked for yet, behind the rest, or
  // ahead of them when `first`.
  const add = (rt: Paircaster, seq: number, jobs: readonly Job[], first = false) => {
    const fresh = jobs.filter((j) => !asked.current.has(jobKey(j)));
    for (const j of fresh) asked.current.add(jobKey(j));
    if (first) queue.current.unshift(...fresh);
    else queue.current.push(...fresh);
    pump(rt, seq);
  };

  const restart = (seq: number) => {
    asked.current = new Set();
    queue.current = [];
    workers.current = { seq, n: 0 };
  };

  return { add, restart, asked: (): ReadonlySet<string> => asked.current, serving: (seq: number) => workers.current.seq === seq };
}

// A drawn cast's jobs: its counts, and the casts it quotes (in a list, also
// the one it replies to) that the page does not hold.
const jobsOf = (casts: readonly Cast[], list: boolean, held: Contexts): Job[] =>
  casts.flatMap((c) => [
    { read: "counts" as const, ref: refOf(c) },
    ...wantedOf([c], list)
      .filter((r) => !held.has(r))
      .map((ref) => ({ read: "show" as const, ref })),
  ]);

function runJob(rt: Paircaster, job: Job, current: () => boolean, store: Pick<Store, "setTallies" | "branches" | "fetched" | "profiles">): Promise<unknown> {
  switch (job.read) {
    case "counts":
      return Promise.all([readReactions(rt, job.ref, current, store.setTallies), readReplies(rt, job.ref, current, store.branches)]);
    case "show":
      return readShow(rt, job.ref, current, store.fetched);
    case "profile":
      return readProfile(rt, job.fid, current, store.profiles);
    default: {
      const unknown: never = job;
      throw new Error(`no read for ${JSON.stringify(unknown)}`);
    }
  }
}

// State that reads outside a render also see: `ref` is current as soon as
// `keep` is called, `value` from the next render.
interface Kept<T> {
  value: T;
  ref: { readonly current: T };
  keep(next: T): void;
}

function useKept<T>(initial: T): Kept<T> {
  const [value, setValue] = useState(initial);
  const ref = useRef(initial);
  const keep = (next: T) => {
    ref.current = next;
    setValue(next);
  };
  return { value, ref, keep };
}

const readReplies = async (rt: Paircaster, ref: string, current: () => boolean, branches: Kept<Map<string, Branch>>) => {
  if (branches.ref.current.has(ref)) return;
  const r = await rt.replies(ref, { limit: BRANCH_READ }).catch(() => null);
  if (r && current()) branches.keep(new Map(branches.ref.current).set(ref, branchOf(r)));
};

const readReactions = async (rt: Paircaster, ref: string, current: () => boolean, set: (f: (t: Map<string, Tally>) => Map<string, Tally>) => void) => {
  const r = await rt.reactions(ref).catch(() => null);
  const tally = r && { likes: r.likes.length, recasts: r.recasts.length };
  if (tally && current()) set((t) => new Map(t).set(ref, tally));
};

// A cast the node refuses is drawn as gone: deleted, or pruned.
const readShow = async (rt: Paircaster, ref: string, current: () => boolean, fetched: Kept<Map<string, Context | null>>) => {
  if (fetched.ref.current.has(ref)) return;
  const r = await rt.show(ref).catch(() => null);
  const context = r && { cast: r.cast, account: r.accounts.find((a) => a.fid === r.cast.fid) };
  if (current()) fetched.keep(new Map(fetched.ref.current).set(ref, context));
};

// A profile the node refuses leaves the card with what the page holds.
const readProfile = async (rt: Paircaster, fid: number, current: () => boolean, profiles: Kept<Profiles>) => {
  if (profiles.ref.current.has(fid)) return;
  const r = await rt.profile(fid).catch(() => null);
  if (current()) profiles.keep(new Map(profiles.ref.current).set(fid, r));
};
