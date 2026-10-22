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

// The pair-ui host in a browser, a framing host such as Shala, or nothing: a
// page opened as a plain file has no host.
const host = await connect();

type View = { thread: Thread | null; error: string | null; loading: boolean };

// Where the page is, as Shala keeps it across restarts.
type Place = { node: string; cast: string; trail: string[] };

// What the host keeps: the place, and what the owner last chose.
type Kept = Place & { user_selected: string | null };

// What the page shows, read by the message handlers, which outlive a render.
// `userSelected` is the moniker of what the owner last clicked or opened, and
// only that: what a tell or reveal shows does not change it.
type Live = {
  node: string;
  thread: Thread | null;
  selected: string | null;
  trail: string[];
  userSelected: string | null;
};

function App() {
  const [view, setView] = useState<View>({ thread: null, error: null, loading: false });
  const [trail, setTrail] = useState<string[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const runtime = useRef<Selfcast | null>(null);
  const latest = useRef(0);
  const live = useRef<Live>({ node: DEFAULT_NODE, thread: null, selected: null, trail: [], userSelected: null });
  live.current = { ...live.current, thread: view.thread, selected, trail };

  // Shows `place` with `chosen` selected, or throws and changes nothing; a
  // load overtaken by a later one changes nothing and returns null.
  const load = async (place: Place, chosen: string | null = null): Promise<Thread | null> => {
    const seq = ++latest.current;
    if (runtime.current?.node !== place.node) runtime.current = makeRuntime(place.node);
    setView((v) => ({ ...v, loading: true }));
    try {
      const thread = await runtime.current.thread(place.cast);
      if (chosen && !all(thread).some((c) => refOf(c) === chosen)) {
        throw new Error(`${chosen} is not in the thread of ${place.cast}`);
      }
      if (seq !== latest.current) return null;
      live.current = { ...live.current, node: place.node, thread, selected: chosen, trail: place.trail };
      setView({ thread, error: null, loading: false });
      setTrail(place.trail);
      setSelected(chosen);
      save(live.current);
      window.scrollTo(0, 0);
      return thread;
    } catch (e) {
      if (seq === latest.current) setView((v) => ({ ...v, loading: false }));
      throw e;
    }
  };

  useEffect(() => {
    if (!host) return;
    const kept = host.kept;
    const restored = isKept(kept)
      ? ((live.current.userSelected = kept.user_selected), load(kept).catch(() => {}))
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
  const point = (thread: Thread, chosen: string | null) => {
    const focus = refOf(thread.cast);
    live.current.userSelected = monikerOf(focus) + (chosen && chosen !== focus ? `#cast=${chosen}` : "");
    save(live.current);
  };

  const open = async (ref: string, back = false) => {
    if (!view.thread) return;
    const from = refOf(view.thread.cast);
    const next = back ? trail.slice(0, -1) : [...trail, from];
    try {
      const thread = await load({ node: live.current.node, cast: ref, trail: next });
      if (thread) point(thread, null);
    } catch (e) {
      setView((v) => ({ ...v, error: String(e instanceof Error ? e.message : e) }));
    }
  };

  const choose = (cast: Cast) => {
    if (!view.thread || selected === refOf(cast)) return;
    setSelected(refOf(cast));
    point(view.thread, refOf(cast));
  };

  if (!host) return <p class="note">Open this page with <code>selfcast viewer open</code>.</p>;
  if (!view.thread) return <p class="note">{view.error ?? (view.loading ? "Reading the node…" : "No thread open yet.")}</p>;
  return (
    <main class={view.loading ? "loading" : ""}>
      {trail.length > 0 && (
        <button class="back" onClick={() => open(trail[trail.length - 1], true)}>
          ← Back
        </button>
      )}
      {view.error && <p class="error">{view.error}</p>}
      <ThreadView thread={view.thread} selected={selected} onOpen={(ref) => open(ref)} onSelect={choose} />
    </main>
  );
}

const SCHEME = "selfcast";
const monikerOf = (cast: string) => `${SCHEME}://${host!.project}/thread/${cast}`;

// `selfcast://<project>/thread/<FID:HASH>`, with `#cast=<FID:HASH>` for a
// cast selected in it.
function parseMoniker(moniker: string): { cast: string; chosen: string | null } | null {
  const prefix = `${SCHEME}://${host!.project}/thread/`;
  if (!moniker.startsWith(prefix)) return null;
  const [cast, fragment] = moniker.slice(prefix.length).split("#", 2);
  const chosen = fragment?.startsWith("cast=") ? fragment.slice(5) : null;
  return cast ? { cast, chosen } : null;
}

async function reveal(
  load: (place: Place, chosen: string | null) => Promise<Thread | null>,
  now: Live,
  moniker: string,
  state: unknown,
): Promise<Answer> {
  const target = parseMoniker(moniker);
  if (!target) return { ok: false, message: `not a thread in this project: ${moniker}` };
  const node = isObject(state) && typeof state.node === "string" ? state.node : now.node;
  const thread = await load({ node, cast: target.cast, trail: [] }, target.chosen);
  if (!thread) return { ok: false, message: `${target.cast} was overtaken by a later reveal; this one changed nothing` };
  const counts = `${count(thread.ancestors.length, "cast")} above, ${count(thread.replies.length, "reply", "replies")}, from ${node}`;
  return { ok: true, message: `thread of ${target.cast}${target.chosen ? `, ${target.chosen} selected` : ""}: ${counts}` };
}

async function tell(now: Live, body: unknown): Promise<Answer> {
  if (!isObject(body) || body.selfcast !== PROTOCOL) {
    return { ok: false, message: `this page takes selfcast: ${PROTOCOL}; run the selfcast that built it` };
  }
  if (body.get !== "state") return { ok: false, message: `unknown request ${JSON.stringify(body)}` };
  const common = { node: now.node, user_selected: now.userSelected };
  if (!now.thread) return { ok: true, message: "no thread open", result: { view: null, ...common } };
  const [text, state] = describe(now.thread, now.selected, now.trail);
  return { ok: true, message: text, result: { ...state, ...common } };
}

const count = (n: number, one: string, many = one + "s") => `${n} ${n === 1 ? one : many}`;

const isObject = (v: unknown): v is Record<string, unknown> => typeof v === "object" && v !== null;

function save(now: Live) {
  if (!now.thread) return;
  const kept: Kept = { node: now.node, cast: refOf(now.thread.cast), trail: now.trail, user_selected: now.userSelected };
  host?.keep(kept);
}

const isKept = (v: unknown): v is Kept =>
  isPlace(v) && "user_selected" in v && (v.user_selected === null || typeof v.user_selected === "string");

const isPlace = (v: unknown): v is Place =>
  isObject(v) && typeof v.node === "string" && typeof v.cast === "string" && Array.isArray(v.trail);

const all = (thread: Thread) => [...thread.ancestors, thread.cast, ...thread.replies];

// One line naming a cast, for the owner and the agent.
function about(thread: Thread, c: Cast): string {
  const who = thread.accounts.find((a) => a.fid === c.fid)?.username;
  const words = (c.display_text ?? c.text).replace(/\s+/g, " ").slice(0, 80);
  return `${refOf(c)} by ${who ? "@" + who : "fid " + c.fid} ("${words}")`;
}

type Actions = {
  selected: string | null;
  onOpen(ref: string): void;
  onSelect(cast: Cast): void;
};

function ThreadView({ thread, ...actions }: { thread: Thread } & Actions) {
  const accounts = new Map(thread.accounts.map((a) => [a.fid, a]));
  const now = Date.now();
  const card = (cast: Cast, role: Role) => (
    <CastCard key={cast.hash} cast={cast} role={role} accounts={accounts} now={now} {...actions} />
  );
  return (
    <>
      <Top top={thread.top} onOpen={actions.onOpen} />
      {thread.ancestors.map((c) => card(c, "ancestor"))}
      {card(thread.cast, "focus")}
      <h2 class="replies">{count(thread.replies.length, "reply", "replies")}</h2>
      {thread.replies.map((c) => card(c, "reply"))}
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

function CastCard({ cast, role, accounts, now, selected, onOpen, onSelect }: CardProps) {
  const account = accounts.get(cast.fid);
  const ref = refOf(cast);
  const click = () => (role === "focus" ? onSelect(cast) : onOpen(ref));
  return (
    <article
      class={`cast ${role}${selected === ref ? " selected" : ""}`}
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
      </div>
    </article>
  );
}

function Avatar({ account, fid }: { account: Account | undefined; fid: number }) {
  return (
    <div class="avatar" style={{ "--hue": hue(fid) }} aria-hidden="true">
      {initials(account, fid)}
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

function Embeds({ cast, onOpen }: { cast: Cast; onOpen(ref: string): void }) {
  if (cast.embeds.length === 0) return null;
  return (
    <div class="embeds">
      {cast.embeds.map((e, i) =>
        "url" in e ? (
          <Link key={i} url={e.url} chip>
            {hostOf(e.url)}
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

function hostOf(url: string): string {
  try {
    return new URL(url).host.replace(/^www\./, "");
  } catch {
    return url;
  }
}

// What the page shows: one line, and the same as data.
function describe(thread: Thread, selected: string | null, back: string[]): [string, Record<string, unknown>] {
  const chosen = all(thread).find((c) => refOf(c) === selected);
  const text =
    `the thread of ${about(thread, thread.cast)}, ${count(thread.ancestors.length, "cast")} above it, ` +
    count(thread.replies.length, "reply", "replies") +
    (chosen ? `; selected ${about(thread, chosen)}` : "");
  return [text, { view: "thread", cast: refOf(thread.cast), selected, back }];
}

render(<App />, document.getElementById("app")!);
