import { render } from "preact";
import { useEffect, useRef, useState } from "preact/hooks";

import { makeRuntime, type Selfcast } from "./runtime";
import { type Answer, connect } from "pair-ui/page";
import {
  type Account,
  type Cast,
  type Thread,
  ago,
  appName,
  hue,
  initials,
  refOf,
  segments,
} from "./thread";

// The node the CLI reads when --node is not given.
const DEFAULT_NODE = "http://localhost:3381";
// The tell bodies this build takes: `{ selfcast: 1, get: "state" }`.
const PROTOCOL = 1;
// The most casts one moniker may name.
const MAX_LISTED = 50;
// How many casts' reactions are read at once.
const COUNTING = 6;

// The pair-ui host in a browser, a framing host such as Shala, or nothing: a
// page opened as a plain file has no host.
const host = await connect();

// What a moniker names, after `selfcast://<project>/`: `thread/<cast>`, with
// `?only=<reply>,…` to show only those replies, in that order; or
// `casts/<cast>,…`, casts from anywhere, in that order. Either may end
// `#cast=<cast>,…` to mark casts it shows.
type Spot = { kind: "thread"; cast: string; only: string[] | null } | { kind: "casts"; casts: string[] };

// What a spot read from the node.
type Shown =
  | { kind: "thread"; thread: Thread; only: string[] | null }
  | { kind: "casts"; casts: Cast[]; accounts: Account[] };

type View = { shown: Shown | null; error: string | null; loading: boolean };

type Tally = { likes: number; recasts: number };

// Where the page is, as the host keeps it across restarts. `at` and each of
// `trail` are what `selfcast viewer open` takes: a cast for a whole thread,
// else a moniker.
type Place = { node: string; at: string; trail: string[] };

// What the host keeps: the place, and what the owner last chose.
type Kept = Place & { user_selected: string | null };

// What the page shows, read by the message handlers, which outlive a render.
// `userSelected` is the moniker of what the owner last clicked or opened, and
// only that: what a tell or reveal shows does not change it.
type Live = {
  node: string;
  spot: Spot | null;
  shown: Shown | null;
  marked: string[];
  trail: string[];
  userSelected: string | null;
};

function App() {
  const [view, setView] = useState<View>({ shown: null, error: null, loading: false });
  const [trail, setTrail] = useState<string[]>([]);
  const [marked, setMarked] = useState<string[]>([]);
  const [tallies, setTallies] = useState<Map<string, Tally>>(new Map());
  const runtime = useRef<Selfcast | null>(null);
  const latest = useRef(0);
  const live = useRef<Live>({ node: DEFAULT_NODE, spot: null, shown: null, marked: [], trail: [], userSelected: null });
  live.current = { ...live.current, shown: view.shown, marked, trail };

  // Shows `at` with `marks` marked, or throws and changes nothing; a load
  // overtaken by a later one changes nothing and returns null.
  const load = async (place: Place): Promise<Shown | null> => {
    const { spot, marks } = parseAt(place.at);
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
  const countReactions = (rt: Selfcast, casts: Cast[], seq: number) => {
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
    if (!host) return;
    const kept = host.kept;
    const restored = isKept(kept)
      ? ((live.current.userSelected = kept.user_selected), load(placeOf(kept)).catch(() => {}))
      : Promise.resolve();
    restored.then(() =>
      host.start({
        reveal: (moniker, state) => reveal(load, live.current, moniker, state),
        tell: (body) => tell(live.current, body),
      }),
    );
  }, []);

  // What the owner does becomes their selection, in the page's state, which
  // a tell reads under either host. Never what a tell or reveal did.
  const point = (spot: Spot, chosen: string | null) => {
    const focus = spot.kind === "thread" ? spot.cast : null;
    live.current.userSelected = monikerOf(spot) + (chosen && chosen !== focus ? `#cast=${chosen}` : "");
    save(live.current);
  };

  const go = async (at: string, back = false) => {
    const from = live.current.spot;
    if (!from) return;
    const next = back ? trail.slice(0, -1) : [...trail, atOf(from)];
    try {
      const shown = await load({ node: live.current.node, at, trail: next });
      if (shown) point(parseAt(at).spot, null);
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

  if (!host) return <p class="note">Open this page with <code>selfcast viewer open</code>.</p>;
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

const SCHEME = "selfcast";
const CAST = /^\d+:0x[0-9a-f]{40}$/i;
const base = () => `${SCHEME}://${host!.project}/`;

function monikerOf(spot: Spot): string {
  if (spot.kind === "casts") return `${base()}casts/${spot.casts.join(",")}`;
  return `${base()}thread/${spot.cast}${spot.only ? `?only=${spot.only.join(",")}` : ""}`;
}

// A whole thread by its cast, the form `selfcast viewer open` takes most.
const atOf = (spot: Spot) => (spot.kind === "thread" && !spot.only ? spot.cast : monikerOf(spot));

// A cast or a moniker, into what it shows and marks; throws on anything else.
function parseAt(at: string): { spot: Spot; marks: string[] } {
  if (CAST.test(at)) return { spot: { kind: "thread", cast: at, only: null }, marks: [] };
  if (!at.startsWith(base())) throw new Error(`not a cast or a moniker of this project: ${at}`);
  const [body, fragment] = cut(at.slice(base().length), "#");
  const [path, query] = cut(body, "?");
  const marks = fragment === null ? [] : listAfter(fragment, "cast=", at);
  if (path.startsWith("casts/")) {
    if (query !== null) throw new Error(`a list of casts takes no ?${query}: ${at}`);
    return { spot: { kind: "casts", casts: list(path.slice(6), at) }, marks };
  }
  const cast = path.startsWith("thread/") ? path.slice(7) : "";
  if (!CAST.test(cast)) throw new Error(`not a thread or a list of casts: ${at}`);
  const only = query === null ? null : listAfter(query, "only=", at);
  return { spot: { kind: "thread", cast, only }, marks };
}

function cut(text: string, mark: string): [string, string | null] {
  const i = text.indexOf(mark);
  return i < 0 ? [text, null] : [text.slice(0, i), text.slice(i + 1)];
}

function listAfter(text: string, key: string, at: string): string[] {
  if (!text.startsWith(key)) throw new Error(`expected ${key}<cast>,… in ${at}`);
  return list(text.slice(key.length), at);
}

function list(text: string, at: string): string[] {
  const casts = text.split(",");
  const bad = casts.find((c) => !CAST.test(c));
  if (bad !== undefined) throw new Error(`not a cast (FID:HASH): "${bad}" in ${at}`);
  if (casts.length > MAX_LISTED) throw new Error(`more than ${MAX_LISTED} casts in ${at}`);
  return casts;
}

// A cast's thread, with any failure naming the cast, so a refused list says
// which of its casts the node could not give.
function threadOf(runtime: Selfcast, cast: string, options?: { limit?: number }): Promise<Thread> {
  return runtime.thread(cast, options).catch((e: unknown) => {
    throw new Error(`${cast}: ${e instanceof Error ? e.message : String(e)}`);
  });
}

async function read(runtime: Selfcast, spot: Spot): Promise<Shown> {
  if (spot.kind === "casts") {
    const threads = await Promise.all(spot.casts.map((c) => threadOf(runtime, c, { limit: 1 })));
    const accounts = new Map(threads.flatMap((t) => t.accounts).map((a) => [a.fid, a]));
    return { kind: "casts", casts: threads.map((t) => t.cast), accounts: [...accounts.values()] };
  }
  const thread = await threadOf(runtime, spot.cast);
  const stray = spot.only?.find((r) => !thread.replies.some((c) => refOf(c) === r));
  if (stray) throw new Error(`${stray} is not a reply to ${spot.cast}`);
  return { kind: "thread", thread, only: spot.only };
}

// The casts on the page, in the order shown.
function visible(shown: Shown): Cast[] {
  if (shown.kind === "casts") return shown.casts;
  const { ancestors, cast } = shown.thread;
  return [...ancestors, cast, ...repliesShown(shown.thread, shown.only)];
}

const repliesShown = (thread: Thread, only: string[] | null) =>
  only ? only.map((r) => thread.replies.find((c) => refOf(c) === r)!) : thread.replies;

const accountsOf = (shown: Shown) => (shown.kind === "thread" ? shown.thread.accounts : shown.accounts);

async function reveal(load: (place: Place) => Promise<Shown | null>, now: Live, moniker: string, state: unknown): Promise<Answer> {
  if (!moniker.startsWith(base())) return { ok: false, message: `not a moniker of this project: ${moniker}` };
  const node = isObject(state) && typeof state.node === "string" ? state.node : now.node;
  const shown = await load({ node, at: moniker, trail: [] });
  if (!shown) return { ok: false, message: `${moniker} was overtaken by a later reveal; this one changed nothing` };
  const marked = parseAt(moniker).marks;
  const marks = marked.length > 0 ? `, ${marked.join(", ")} selected` : "";
  if (shown.kind === "casts") return { ok: true, message: `${count(shown.casts.length, "cast")}${marks}, from ${node}` };
  const { thread, only } = shown;
  const what = `thread of ${refOf(thread.cast)}${only ? `, ${only.length} of its replies` : ""}${marks}`;
  const counts = `${count(thread.ancestors.length, "cast")} above, ${count(thread.replies.length, "reply", "replies")}, from ${node}`;
  return { ok: true, message: `${what}: ${counts}` };
}

async function tell(now: Live, body: unknown): Promise<Answer> {
  if (!isObject(body) || body.selfcast !== PROTOCOL) {
    return { ok: false, message: `this page takes selfcast: ${PROTOCOL}; run the selfcast that built it` };
  }
  if (body.get !== "state") return { ok: false, message: `unknown request ${JSON.stringify(body)}` };
  const common = { node: now.node, user_selected: now.userSelected };
  if (!now.shown) return { ok: true, message: "no thread open", result: { view: null, ...common } };
  const [text, state] = describe(now.shown, now.marked, now.trail);
  return { ok: true, message: text, result: { ...state, ...common } };
}

const count = (n: number, one: string, many = one + "s") => `${n} ${n === 1 ? one : many}`;

const isObject = (v: unknown): v is Record<string, unknown> => typeof v === "object" && v !== null;

function save(now: Live) {
  if (!now.spot) return;
  const kept: Kept = { node: now.node, at: atOf(now.spot), trail: now.trail, user_selected: now.userSelected };
  host?.keep(kept);
}

// Builds before monikers kept `cast` where `at` is now.
const placeOf = (k: Kept & { cast?: string }): Place => ({ node: k.node, at: k.at ?? k.cast!, trail: k.trail });

const isKept = (v: unknown): v is Kept =>
  isObject(v) &&
  typeof v.node === "string" &&
  (typeof v.at === "string" || typeof v.cast === "string") &&
  Array.isArray(v.trail) &&
  (v.user_selected === null || typeof v.user_selected === "string");

// One line naming a cast, for the owner and the agent.
function about(accounts: Account[], c: Cast): string {
  const who = accounts.find((a) => a.fid === c.fid)?.username;
  const words = (c.display_text ?? c.text).replace(/\s+/g, " ").slice(0, 80);
  return `${refOf(c)} by ${who ? "@" + who : "fid " + c.fid} ("${words}")`;
}

type Actions = {
  marked: string[];
  tallies: Map<string, Tally>;
  onOpen(ref: string): void;
  onSelect(cast: Cast): void;
};

type ThreadProps = { thread: Thread; only: string[] | null; onShowAll(): void } & Actions;

function ThreadView({ thread, only, onShowAll, ...actions }: ThreadProps) {
  const accounts = new Map(thread.accounts.map((a) => [a.fid, a]));
  const now = Date.now();
  const card = (cast: Cast, role: Role) => (
    <CastCard key={cast.hash} cast={cast} role={role} accounts={accounts} now={now} {...actions} />
  );
  const all = count(thread.replies.length, "reply", "replies");
  const hidden = only ? thread.replies.length - only.length : 0;
  return (
    <>
      <Top top={thread.top} onOpen={actions.onOpen} />
      {thread.ancestors.map((c) => card(c, "ancestor"))}
      {card(thread.cast, "focus")}
      <h2 class="replies">{only ? `${only.length} of ${all}` : all}</h2>
      {repliesShown(thread, only).map((c) => card(c, "reply"))}
      {hidden > 0 && (
        <button class="note link" onClick={onShowAll}>
          Show all {all}
        </button>
      )}
    </>
  );
}

function ListView({ casts, accounts, ...actions }: { casts: Cast[]; accounts: Account[] } & Actions) {
  const byFid = new Map(accounts.map((a) => [a.fid, a]));
  const now = Date.now();
  return (
    <>
      <h2 class="replies">{count(casts.length, "cast")}</h2>
      {casts.map((c) => (
        <CastCard key={c.hash} cast={c} role="reply" accounts={byFid} now={now} {...actions} />
      ))}
    </>
  );
}

function Top({ top, onOpen }: { top: Thread["top"]; onOpen(ref: string): void }) {
  if (top === "root") return null;
  if ("missing" in top) {
    return <p class="note">Replies to a cast this node does not hold ({refOf(top.missing)}).</p>;
  }
  return (
    <button class="note link" onClick={() => onOpen(refOf(top.cut))}>
      Earlier casts in this thread…
    </button>
  );
}

type Role = "ancestor" | "focus" | "reply";

type CardProps = { cast: Cast; role: Role; accounts: Map<number, Account>; now: number } & Actions;

function CastCard({ cast, role, accounts, now, marked, tallies, onOpen, onSelect }: CardProps) {
  const account = accounts.get(cast.fid);
  const ref = refOf(cast);
  const click = () => (role === "focus" ? onSelect(cast) : onOpen(ref));
  return (
    <article
      class={`cast ${role}${marked.includes(ref) ? " selected" : ""}`}
      data-cast={ref}
      tabIndex={0}
      onClick={click}
      onFocus={() => onSelect(cast)}
      onKeyDown={(e) => e.key === "Enter" && click()}
    >
      <div class="rail">
        <Avatar account={account} fid={cast.fid} />
        {role === "ancestor" && <div class="line" />}
      </div>
      <div class="body">
        <Meta cast={cast} account={account} now={now} />
        <Text cast={cast} clamp={role !== "focus"} />
        <Embeds cast={cast} onOpen={onOpen} />
        <Counts tally={tallies.get(ref)} />
      </div>
    </article>
  );
}

// The picture loads only where the page policy allows remote images
// (`viewer serve --images`); refused or broken, it falls back to initials.
function Avatar({ account, fid }: { account: Account | undefined; fid: number }) {
  const url = account?.pfp_url?.startsWith("https://") ? account.pfp_url : null;
  const [failed, setFailed] = useState<string | null>(null);
  return (
    <div class="avatar" style={{ "--hue": hue(fid) }} aria-hidden="true">
      {url && failed !== url ? (
        <img src={url} alt="" loading="lazy" referrerpolicy="no-referrer" onError={() => setFailed(url)} />
      ) : (
        initials(account, fid)
      )}
    </div>
  );
}

function Meta({ cast, account, now }: { cast: Cast; account: Account | undefined; now: number }) {
  const full = new Date(cast.timestamp).toLocaleString();
  return (
    <header class="meta">
      <span class="name">{account?.display_name ?? account?.username ?? `fid ${cast.fid}`}</span>
      {account?.username && <span class="handle">@{account.username}</span>}
      <span class="dim">·</span>
      <time dateTime={cast.timestamp} title={full}>
        {ago(cast.timestamp, now)}
      </time>
      <span class="app" title={cast.signer}>
        {appName(cast.app_fid)}
      </span>
    </header>
  );
}

// Where the host opens no links (Shala's sandbox), a link shows its address
// and stops there.
function Text({ cast, clamp }: { cast: Cast; clamp: boolean }) {
  const [open, setOpen] = useState(!clamp);
  const long = (cast.display_text ?? cast.text).split("\n").length > 12 || cast.text.length > 700;
  return (
    <>
      <p class={`text${open || !long ? "" : " clamped"}`}>
        {segments(cast).map((s, i) =>
          s.kind === "link" ? (
            <Link key={i} url={s.url}>
              {s.text}
            </Link>
          ) : s.kind === "mention" ? (
            <span key={i} class="mention">
              {s.text}
            </span>
          ) : (
            s.text
          ),
        )}
      </p>
      {long && !open && (
        <button class="more" onClick={(e) => (e.stopPropagation(), setOpen(true))}>
          Show more
        </button>
      )}
    </>
  );
}

// Likes and recasts as the node holds them now; nothing until they are read.
function Counts({ tally }: { tally: Tally | undefined }) {
  if (!tally) return null;
  return (
    <footer class="tally">
      <span title="likes">♥ {tally.likes}</span>
      <span title="recasts">⇄ {tally.recasts}</span>
    </footer>
  );
}

function Embeds({ cast, onOpen }: { cast: Cast; onOpen(ref: string): void }) {
  if (cast.embeds.length === 0) return null;
  return (
    <div class="embeds">
      {cast.embeds.map((e, i) =>
        "url" in e ? (
          <Link key={i} url={e.url} chip>
            {isImage(e.url) ? "image" : hostOf(e.url)}
          </Link>
        ) : (
          <button key={i} class="chip" onClick={(ev) => (ev.stopPropagation(), onOpen(refOf(e.cast)))}>
            quoted cast · fid {e.cast.fid}
          </button>
        ),
      )}
    </div>
  );
}

function Link({ url, chip, children }: { url: string; chip?: boolean; children: preact.ComponentChildren }) {
  const opens = host?.opensLinks ?? false;
  return (
    <a
      class={chip ? "chip" : undefined}
      href={url}
      title={url}
      target="_blank"
      rel="noopener noreferrer"
      onClick={(e) => (opens ? e.stopPropagation() : (e.preventDefault(), e.stopPropagation()))}
    >
      {children}
    </a>
  );
}

// The page shows no images in casts, so an image is a link labelled as one:
// Farcaster's image host, or a link to an image file.
function isImage(url: string): boolean {
  try {
    const u = new URL(url);
    return u.host === "imagedelivery.net" || /\.(png|jpe?g|gif|webp|avif)$/i.test(u.pathname);
  } catch {
    return false;
  }
}

function hostOf(url: string): string {
  try {
    return new URL(url).host.replace(/^www\./, "");
  } catch {
    return url;
  }
}

// What the page shows: one line, and the same as data.
function describe(shown: Shown, marked: string[], back: string[]): [string, Record<string, unknown>] {
  const accounts = accountsOf(shown);
  const named = (refs: string[]) =>
    refs.map((r) => about(accounts, visible(shown).find((c) => refOf(c) === r)!)).join("; ");
  const selected = marked.length > 0 ? marked : null;
  const marks = selected ? `; selected ${named(marked)}` : "";
  if (shown.kind === "casts") {
    const casts = shown.casts.map(refOf);
    return [`${count(casts.length, "cast")}: ${named(casts)}${marks}`, { view: "casts", casts, selected, back }];
  }
  const { thread, only } = shown;
  const text =
    `the thread of ${about(accounts, thread.cast)}, ${count(thread.ancestors.length, "cast")} above it, ` +
    count(thread.replies.length, "reply", "replies") +
    (only ? `, showing only ${named(only)}` : "") +
    marks;
  const filter = only ? { only } : {};
  return [text, { view: "thread", cast: refOf(thread.cast), ...filter, selected, back }];
}

render(<App />, document.getElementById("app")!);
