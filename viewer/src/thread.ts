// The `thread` tool's result: `selfcast thread --json`, version 1
// (crates/cli/src/reads.rs, `ThreadV1`).

export type Ref = { fid: number; hash: string };
export type Target = { cast: Ref } | { url: string };

export type Cast = {
  hash: string;
  fid: number;
  timestamp: string;
  text: string;
  display_text: string | null;
  mentions: { fid: number; username: string | null; position: number }[];
  embeds: Target[];
  parent: Target | null;
  signer: string;
  app_fid: number | null;
};

export type Account = { fid: number; username: string | null; display_name: string | null };

export type Thread = {
  version: 1;
  top: "root" | { missing: Ref } | { cut: Ref };
  ancestors: Cast[];
  cast: Cast;
  replies: Cast[];
  accounts: Account[];
};

export function asThread(value: unknown): Thread {
  const t = value as Thread;
  if (t?.version !== 1 || !t.cast || !Array.isArray(t.replies)) {
    throw new Error("not a version 1 thread");
  }
  return t;
}

export const refOf = (c: { fid: number; hash: string }) => `${c.fid}:${c.hash}`;

// The apps Snapchain itself names (v0.14.3,
// `src/connectors/onchain_events/mod.rs:255-260`).
const APPS: Record<number, string> = { 9152: "Farcaster", 309857: "Base app" };

export function appName(fid: number | null): string {
  if (fid === null) return "unknown app";
  return APPS[fid] ?? `app ${fid}`;
}

export type Segment =
  | { kind: "text"; text: string }
  | { kind: "link"; text: string; url: string }
  | { kind: "mention"; text: string };

const TOKEN = /https?:\/\/[^\s<>"]+|@fid:\d+|@[a-z0-9][a-z0-9_.-]*/gi;

// Splits a cast's text into plain text, links and mentions. Farcaster marks
// mentions but not links, so links are found by pattern; a mention counts
// only when the cast names that account.
export function segments(cast: Cast): Segment[] {
  const text = cast.display_text ?? cast.text;
  const names = new Set(cast.mentions.map((m) => (m.username ?? `fid:${m.fid}`).toLowerCase()));
  const out: Segment[] = [];
  let from = 0;
  for (const match of text.matchAll(TOKEN)) {
    const token = trimLink(match[0]);
    const at = match.index;
    const isLink = token.startsWith("http");
    if (!isLink && !names.has(token.slice(1).toLowerCase())) continue;
    if (at > from) out.push({ kind: "text", text: text.slice(from, at) });
    out.push(isLink ? { kind: "link", text: shortUrl(token), url: token } : { kind: "mention", text: token });
    from = at + token.length;
  }
  if (from < text.length) out.push({ kind: "text", text: text.slice(from) });
  return out;
}

// A link's trailing punctuation is the sentence's, and so is a closing
// bracket the link did not open.
function trimLink(token: string): string {
  if (!token.startsWith("http")) return token.replace(/[.]+$/, "");
  let t = token.replace(/[.,;:!?'"]+$/, "");
  if (t.endsWith(")") && !t.includes("(")) t = t.slice(0, -1);
  return t;
}

export function shortUrl(url: string): string {
  try {
    const u = new URL(url);
    const host = u.host.replace(/^www\./, "");
    const rest = u.pathname + u.search;
    return rest.length > 1 ? host + (rest.length > 16 ? rest.slice(0, 15) + "…" : rest) : host;
  } catch {
    return url;
  }
}

const MINUTE = 60;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

export function ago(timestamp: string, now: number): string {
  const then = Date.parse(timestamp);
  const s = Math.max(0, Math.floor((now - then) / 1000));
  if (s < MINUTE) return `${s}s`;
  if (s < HOUR) return `${Math.floor(s / MINUTE)}m`;
  if (s < DAY) return `${Math.floor(s / HOUR)}h`;
  if (s < 30 * DAY) return `${Math.floor(s / DAY)}d`;
  const sameYear = new Date(then).getFullYear() === new Date(now).getFullYear();
  return new Date(then).toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
    ...(sameYear ? {} : { year: "numeric" }),
  });
}

export function initials(account: Account | undefined, fid: number): string {
  const name = account?.display_name ?? account?.username ?? String(fid);
  return Array.from(name.trim())[0]?.toUpperCase() ?? "?";
}

// A stable hue per account, so the same person keeps the same colour.
export function hue(fid: number): number {
  return (fid * 47) % 360;
}
