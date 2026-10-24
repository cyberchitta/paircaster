// One cast on the page, and its parts.

import { useState } from "preact/hooks";

import { type Branch, repliesLabel } from "./branch";
import { host } from "./host";
import { type Account, type Cast, ago, appName, hue, initials, refOf, segments } from "./thread";

export type Tally = { likes: number; recasts: number };

// What a card does when pointed at, and what it shows beside the cast:
// likes and recasts, and its direct replies, each once read.
export type Actions = {
  marked: string[];
  tallies: Map<string, Tally>;
  branches: Map<string, Branch>;
  onOpen(ref: string): void;
  onSelect(cast: Cast): void;
};

export type Role = "ancestor" | "focus" | "reply";

type CardProps = { cast: Cast; role: Role; accounts: Map<number, Account>; now: number } & Actions;

export function CastCard({ cast, role, accounts, now, marked, tallies, branches, onOpen, onSelect }: CardProps) {
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
