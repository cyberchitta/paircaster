// One cast on the page, and its parts.

import { useState } from "preact/hooks";

import { type Branch, repliesLabel } from "./branch";
import { type Context, type Contexts, PARENT_EXCERPT, QUOTE_EXCERPT, authorOf, excerpt, parentOf } from "./context";
import { host } from "./host";
import { type Account, type Cast, ago, appName, hue, initials, refOf, segments } from "./thread";

export type Tally = { likes: number; recasts: number };

// What a card does when pointed at, and what it shows beside the cast:
// likes and recasts, its direct replies, and the casts it quotes or replies
// to, each once read.
export type Actions = {
  marked: string[];
  tallies: Map<string, Tally>;
  branches: Map<string, Branch>;
  contexts: Contexts;
  onOpen(ref: string): void;
  onSelect(cast: Cast): void;
};

export type Role = "ancestor" | "focus" | "reply";

// `nested` for a reply under a reply; `parent` for a card with replies drawn
// under it, joined to them by a line; `replying` for a card that names the
// cast it replies to, which a thread draws above it instead.
type CardProps = {
  cast: Cast;
  role: Role;
  nested?: boolean;
  parent?: boolean;
  replying?: boolean;
  accounts: Map<number, Account>;
  now: number;
} & Actions;

export function CastCard(props: CardProps) {
  const { cast, role, nested, parent, replying, accounts, now, marked, tallies, branches, contexts, onOpen, onSelect } =
    props;
  const account = accounts.get(cast.fid);
  const ref = refOf(cast);
  const click = () => (role === "focus" ? onSelect(cast) : onOpen(ref));
  return (
    <article
      class={["cast", role, nested && "nested", parent && "parent", marked.includes(ref) && "selected"]
        .filter(Boolean)
        .join(" ")}
      data-cast={ref}
      tabIndex={0}
      onClick={click}
      onFocus={() => onSelect(cast)}
      onKeyDown={(e) => e.key === "Enter" && e.target === e.currentTarget && click()}
    >
      <div class="rail">
        <Avatar account={account} fid={cast.fid} />
        {(role === "ancestor" || parent) && <div class="line" />}
      </div>
      <div class="body">
        {replying && <ReplyingTo cast={cast} contexts={contexts} onOpen={onOpen} />}
        <Meta cast={cast} account={account} now={now} />
        <Text cast={cast} clamp={role !== "focus"} />
        <Embeds cast={cast} contexts={contexts} now={now} onOpen={onOpen} />
        <Counts tally={tallies.get(ref)} branch={branches.get(ref)} />
      </div>
    </article>
  );
}

// The picture loads only where the page policy allows remote images
// (`viewer serve --images`); refused or broken, it falls back to initials.
export function Avatar({ account, fid }: { account: Account | undefined; fid: number }) {
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

// Replies, likes and recasts as the node holds them now; each shows once
// read, and replies only when there are some.
function Counts({ tally, branch }: { tally: Tally | undefined; branch: Branch | undefined }) {
  const replies = branch && repliesLabel(branch);
  if (!tally && !replies) return null;
  return (
    <footer class="tally">
      {replies && <span title="direct replies">{replies}</span>}
      {tally && <Reactions tally={tally} />}
    </footer>
  );
}

function Reactions({ tally }: { tally: Tally }) {
  return (
    <>
      <span title="likes">♥ {tally.likes}</span>
      <span title="recasts">⇄ {tally.recasts}</span>
    </>
  );
}

// The cast a list cast replies to, on one line; a click opens its thread.
// Until it is read it names no one; one the node does not hold is named so.
function ReplyingTo({ cast, contexts, onOpen }: { cast: Cast; contexts: Contexts; onOpen(ref: string): void }) {
  const ref = parentOf(cast);
  if (!ref) return null;
  const context = contexts.get(ref);
  if (context === null) return <p class="context gone">replying to a cast that's gone</p>;
  return (
    <button
      class="context"
      title={context ? excerpt(context.cast, 1000) : ref}
      onClick={(e) => (e.stopPropagation(), onOpen(ref))}
    >
      replying to{" "}
      {context ? (
        <>
          <span class="who">{authorOf(context)}</span>: {excerpt(context.cast, PARENT_EXCERPT)}
        </>
      ) : (
        "a cast"
      )}
    </button>
  );
}

// A quoted cast once read: its author and the start of its text; a click
// opens its thread. Until then, or where the node does not hold it, a chip.
function Quote({ fid, target, context, now, onOpen }: {
  fid: number;
  target: string;
  context: Context | null | undefined;
  now: number;
  onOpen(ref: string): void;
}) {
  const open = (e: Event) => (e.stopPropagation(), onOpen(target));
  if (!context) {
    return (
      <button class="chip" onClick={open}>
        quoted cast · fid {fid}
      </button>
    );
  }
  const { cast, account } = context;
  return (
    <button class="quote" onClick={open} title="open the quoted cast">
      <span class="meta">
        <Avatar account={account} fid={cast.fid} />
        <span class="name">{account?.display_name ?? account?.username ?? `fid ${cast.fid}`}</span>
        {account?.username && <span class="handle">@{account.username}</span>}
        <span class="dim">·</span>
        <time dateTime={cast.timestamp}>{ago(cast.timestamp, now)}</time>
      </span>
      <span class="excerpt">{excerpt(cast, QUOTE_EXCERPT)}</span>
    </button>
  );
}

function Embeds({ cast, contexts, now, onOpen }: {
  cast: Cast;
  contexts: Contexts;
  now: number;
  onOpen(ref: string): void;
}) {
  if (cast.embeds.length === 0) return null;
  return (
    <div class="embeds">
      {cast.embeds.map((e, i) =>
        "url" in e ? (
          <Link key={i} url={e.url} chip>
            {isImage(e.url) ? "image" : hostOf(e.url)}
          </Link>
        ) : (
          <Quote
            key={i}
            fid={e.cast.fid}
            target={refOf(e.cast)}
            context={contexts.get(refOf(e.cast))}
            now={now}
            onOpen={onOpen}
          />
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
