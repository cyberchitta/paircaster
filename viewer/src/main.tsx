import { render } from "preact";
import { useEffect, useRef, useState } from "preact/hooks";

import { connect, type Host, type HostContext, inHost, type ToolResult } from "./host";
import { makeRuntime, type Selfcast } from "./runtime";
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

type View = { thread: Thread | null; error: string | null; loading: boolean };

// What the `view` tool hands the page.
type Opened = { cast: string; node: string };

function App() {
  const [view, setView] = useState<View>({ thread: null, error: null, loading: true });
  const [trail, setTrail] = useState<string[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const runtime = useRef<Selfcast | null>(null);

  const load = async (ref: string) => {
    const rt = runtime.current;
    if (!rt) return false;
    setView((v) => ({ ...v, loading: true }));
    try {
      setView({ thread: await rt.thread(ref), error: null, loading: false });
      setSelected(null);
      window.scrollTo(0, 0);
      return true;
    } catch (e) {
      setView((v) => ({ ...v, error: String(e instanceof Error ? e.message : e), loading: false }));
      return false;
    }
  };

  useEffect(() => {
    if (!inHost()) return;
    connect({
      onToolResult: (result: ToolResult, host: Host) => {
        if (result.isError) {
          setView((v) => ({ ...v, error: result.content?.[0]?.text ?? "the tool failed", loading: false }));
          return;
        }
        const opened = result.structuredContent as Opened;
        runtime.current = makeRuntime(host, opened.node);
        setTrail([]);
        load(opened.cast);
      },
      onContext: applyTheme,
    });
  }, []);

  useEffect(() => {
    const rt = runtime.current;
    if (rt && view.thread) rt.setState(...describe(view.thread, selected, trail));
  }, [view.thread, selected, trail]);

  const open = async (ref: string, back = false) => {
    if (!view.thread) return;
    const from = refOf(view.thread.cast);
    if (await load(ref)) setTrail((t) => (back ? t.slice(0, -1) : [...t, from]));
  };

  const select = (cast: Cast) => setSelected(refOf(cast));

  if (!inHost()) return <p class="note">This page runs inside an MCP Apps host, opened by Selfcast's <code>view</code> tool.</p>;
  if (!view.thread) return <p class="note">{view.error ?? "Waiting for a thread…"}</p>;
  return (
    <main class={view.loading ? "loading" : ""}>
      {trail.length > 0 && (
        <button class="back" onClick={() => open(trail[trail.length - 1], true)}>
          ← Back
        </button>
      )}
      {view.error && <p class="error">{view.error}</p>}
      <ThreadView
        thread={view.thread}
        selected={selected}
        onOpen={(ref) => open(ref)}
        onSelect={select}
        onLink={(url) => runtime.current?.openLink(url)}
      />
    </main>
  );
}

type Actions = {
  selected: string | null;
  onOpen(ref: string): void;
  onSelect(cast: Cast, account: Account | undefined): void;
  onLink(url: string): void;
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
      <h2 class="replies">
        {thread.replies.length === 1 ? "1 reply" : `${thread.replies.length} replies`}
      </h2>
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

function CastCard({ cast, role, accounts, now, selected, onOpen, onSelect, onLink }: CardProps) {
  const account = accounts.get(cast.fid);
  const ref = refOf(cast);
  const click = () => (role === "focus" ? onSelect(cast, account) : onOpen(ref));
  return (
    <article
      class={`cast ${role}${selected === ref ? " selected" : ""}`}
      data-cast={ref}
      tabIndex={0}
      onClick={click}
      onFocus={() => onSelect(cast, account)}
      onKeyDown={(e) => e.key === "Enter" && click()}
    >
      <div class="rail">
        <Avatar account={account} fid={cast.fid} />
        {role === "ancestor" && <div class="line" />}
      </div>
      <div class="body">
        <Meta cast={cast} account={account} now={now} />
        <Text cast={cast} clamp={role !== "focus"} onLink={onLink} />
        <Embeds cast={cast} onOpen={onOpen} onLink={onLink} />
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

function Text({ cast, clamp, onLink }: { cast: Cast; clamp: boolean; onLink(url: string): void }) {
  const [open, setOpen] = useState(!clamp);
  const long = (cast.display_text ?? cast.text).split("\n").length > 12 || cast.text.length > 700;
  return (
    <>
      <p class={`text${open || !long ? "" : " clamped"}`}>
        {segments(cast).map((s, i) =>
          s.kind === "link" ? (
            <a key={i} href={s.url} onClick={(e) => (e.preventDefault(), e.stopPropagation(), onLink(s.url))}>
              {s.text}
            </a>
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

function Embeds({ cast, onOpen, onLink }: { cast: Cast; onOpen(ref: string): void; onLink(url: string): void }) {
  if (cast.embeds.length === 0) return null;
  return (
    <div class="embeds">
      {cast.embeds.map((e, i) =>
        "url" in e ? (
          <button key={i} class="chip" title={e.url} onClick={(ev) => (ev.stopPropagation(), onLink(e.url))}>
            {hostOf(e.url)}
          </button>
        ) : (
          <button key={i} class="chip" onClick={(ev) => (ev.stopPropagation(), onOpen(refOf(e.cast)))}>
            quoted cast · fid {e.cast.fid}
          </button>
        ),
      )}
    </div>
  );
}

function hostOf(url: string): string {
  try {
    return new URL(url).host.replace(/^www\./, "");
  } catch {
    return url;
  }
}

// The page's current state, for the model: one line, and the same as data.
function describe(thread: Thread, selected: string | null, back: string[]): [string, Record<string, unknown>] {
  const accounts = new Map(thread.accounts.map((a) => [a.fid, a]));
  const all = [...thread.ancestors, thread.cast, ...thread.replies];
  const about = (c: Cast) => {
    const who = accounts.get(c.fid)?.username;
    const words = (c.display_text ?? c.text).replace(/\s+/g, " ").slice(0, 80);
    return `${refOf(c)} by ${who ? "@" + who : "fid " + c.fid} ("${words}")`;
  };
  const chosen = all.find((c) => refOf(c) === selected);
  const text =
    `Selfcast viewer: the thread of ${about(thread.cast)}, ${thread.ancestors.length} casts above it, ` +
    `${thread.replies.length} replies.` +
    (chosen ? ` The user has selected ${about(chosen)}.` : "");
  return [text, { view: "thread", cast: refOf(thread.cast), selected, back }];
}

function applyTheme(context: HostContext) {
  if (context.theme) document.documentElement.dataset.theme = context.theme;
}

render(<App />, document.getElementById("app")!);
