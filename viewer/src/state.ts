// What the page keeps, and what it answers the agent: a reveal, and a tell
// for `viewer state`. Both answers are external formats (skill/SKILL.md).

import type { Answer } from "pair-ui/page";
import { type Spot, baseOf, parseAt } from "./moniker";
import { type Shown, accountsOf, count, visible } from "./shown";
import { type Account, type Cast, refOf } from "./thread";

// The tell bodies this build takes: `{ paircaster: 1, get: "state" }`.
const PROTOCOL = 1;

// Where the page is, as the host keeps it across restarts. `at` and each of
// `trail` are what `paircaster viewer open` takes: a cast for a whole thread,
// else a moniker.
export type Place = { node: string; at: string; trail: string[] };

// What the host keeps: the place, and what the owner last chose.
export type Kept = Place & { user_selected: string | null };

// What the page shows, read by the message handlers, which outlive a render.
// `userSelected` is the moniker of what the owner last clicked or opened, and
// only that: what a tell or reveal shows does not change it.
export type Live = {
  node: string;
  spot: Spot | null;
  shown: Shown | null;
  marked: string[];
  trail: string[];
  userSelected: string | null;
};

export async function reveal(
  project: string,
  load: (place: Place) => Promise<Shown | null>,
  now: Live,
  moniker: string,
  state: unknown,
): Promise<Answer> {
  if (!moniker.startsWith(baseOf(project))) return { ok: false, message: `not a moniker of this project: ${moniker}` };
  const node = isObject(state) && typeof state.node === "string" ? state.node : now.node;
  const shown = await load({ node, at: moniker, trail: [] });
  if (!shown) return { ok: false, message: `${moniker} was overtaken by a later reveal; this one changed nothing` };
  const marked = parseAt(project, moniker).marks;
  const marks = marked.length > 0 ? `, ${marked.join(", ")} selected` : "";
  if (shown.kind === "casts") return { ok: true, message: `${count(shown.casts.length, "cast")}${marks}, from ${node}` };
  const { thread, only } = shown;
  const what = `thread of ${refOf(thread.cast)}${only ? `, ${only.length} of its replies` : ""}${marks}`;
  const counts = `${count(thread.ancestors.length, "cast")} above, ${count(thread.replies.length, "reply", "replies")}, from ${node}`;
  return { ok: true, message: `${what}: ${counts}` };
}

export async function tell(now: Live, body: unknown): Promise<Answer> {
  if (!isObject(body) || body.paircaster !== PROTOCOL) {
    return { ok: false, message: `this page takes paircaster: ${PROTOCOL}; run the paircaster that built it` };
  }
  if (body.get !== "state") return { ok: false, message: `unknown request ${JSON.stringify(body)}` };
  const common = { node: now.node, user_selected: now.userSelected };
  if (!now.shown) return { ok: true, message: "no thread open", result: { view: null, ...common } };
  const [text, state] = describe(now.shown, now.marked, now.trail);
  return { ok: true, message: text, result: { ...state, ...common } };
}

const isObject = (v: unknown): v is Record<string, unknown> => typeof v === "object" && v !== null;

// Builds before monikers kept `cast` where `at` is now.
export const placeOf = (k: Kept & { cast?: string }): Place => ({ node: k.node, at: k.at ?? k.cast!, trail: k.trail });

export const isKept = (v: unknown): v is Kept =>
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

// What the page shows: one line, and the same as data.
export function describe(shown: Shown, marked: string[], back: string[]): [string, Record<string, unknown>] {
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
