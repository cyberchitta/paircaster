// What the page keeps, and what it answers the agent: a reveal, and a tell
// for `viewer state`. Both answers are external formats (skill/SKILL.md).

import type { Answer } from "pair-ui/page";
import type { Nest } from "./fold";
import { baseOf, parseAt, type Spot } from "./moniker";
import type { Profile } from "./reads";
import { accountsOf, count, onScreen, type Shown } from "./shown";
import { type Account, type Cast, refOf } from "./thread";

// The tell bodies this build takes: `{ paircaster: 1, get: "state" }`.
const PROTOCOL = 1;

// Where the page is, as the host keeps it across restarts. `at` and each of
// `trail` are what `paircaster viewer open` takes: a cast for a whole thread,
// else a moniker.
export type Place = { node: string; at: string; trail: string[] };

// What the host keeps: the place, and what the owner last chose and marked
// (builds before marking several kept no `user_marked`).
export type Kept = Place & { user_selected: string | null; user_marked?: string | null };

// What the page shows, read by the message handlers, which outlive a render.
// `userSelected` is the moniker of what the owner last clicked or opened, and
// only that: what a tell or reveal shows does not change it. `opened` holds
// the folds the owner opened on this view; `nest` the replies read below the
// focus, and how deep the page nests them. `userMarked` names the casts the
// owner marked on the view they are on, as a moniker, or is null.
export type Live = {
  node: string;
  spot: Spot | null;
  shown: Shown | null;
  marked: string[];
  trail: string[];
  opened: ReadonlySet<string>;
  nest: Nest;
  userSelected: string | null;
  userMarked: string | null;
};

export async function reveal(project: string, load: (place: Place) => Promise<Shown | null>, now: Live, moniker: string, state: unknown): Promise<Answer> {
  if (!moniker.startsWith(baseOf(project))) return { ok: false, message: `not a moniker of this project: ${moniker}` };
  const node = isObject(state) && typeof state.node === "string" ? state.node : now.node;
  const shown = await load({ node, at: moniker, trail: [] });
  if (!shown) return { ok: false, message: `${moniker} was overtaken by a later reveal; this one changed nothing` };
  const { spot, marks: marked } = parseAt(project, moniker);
  const marks =
    (marked.length > 0 ? `, ${marked.join(", ")} selected` : "") + (spot.caption !== undefined ? `, captioned ${JSON.stringify(spot.caption)}` : "");
  if (shown.kind === "casts") return { ok: true, message: `${count(shown.casts.length, "cast")}${marks}, from ${node}` };
  if (shown.kind === "profile") {
    const what = `profile of ${nameOf(shown.profile)}, ${count(shown.casts.length, "latest cast")}`;
    return { ok: true, message: `${what}${marks}, from ${node}` };
  }
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
  const common = {
    node: now.node,
    caption: now.shown ? (now.spot?.caption ?? null) : null,
    user_selected: now.userSelected,
    user_marked: now.userMarked,
  };
  if (!now.shown) return { ok: true, message: "no thread open", result: { view: null, ...common } };
  const [text, state] = describe(now.shown, now.marked, now.trail, now.opened, now.nest);
  return { ok: true, message: text, result: { ...state, ...common } };
}

const isObject = (v: unknown): v is Record<string, unknown> => typeof v === "object" && v !== null;

// Builds before monikers kept `cast` where `at` is now.
export function placeOf(k: Kept & { cast?: string }): Place {
  const at = k.at ?? k.cast;
  if (at === undefined) throw new Error("a kept place with neither `at` nor `cast`");
  return { node: k.node, at, trail: k.trail };
}

export const isKept = (v: unknown): v is Kept =>
  isObject(v) &&
  typeof v.node === "string" &&
  (typeof v.at === "string" || typeof v.cast === "string") &&
  Array.isArray(v.trail) &&
  (v.user_selected === null || typeof v.user_selected === "string") &&
  (v.user_marked === undefined || v.user_marked === null || typeof v.user_marked === "string");

// An account as a line names it.
const nameOf = (p: Profile) => (p.username ? `@${p.username} (fid ${p.fid})` : `fid ${p.fid}`);

// One line naming a cast, for the owner and the agent.
function about(accounts: Account[], c: Cast | undefined, ref: string): string {
  if (!c) return ref;
  const who = accounts.find((a) => a.fid === c.fid)?.username;
  const words = (c.display_text ?? c.text).replace(/\s+/g, " ").slice(0, 80);
  return `${refOf(c)} by ${who ? `@${who}` : `fid ${c.fid}`} ("${words}")`;
}

// What the page shows: one line, and the same as data. `folded` lists the
// casts on the page that show only as a fold ("+8 more"), nested ones too,
// as the owner left them: none once each fold is opened. `continued` lists
// the replies whose own replies the page leaves to their thread ("Continue
// this thread"), and `depth` how many levels of replies it nests.
export function describe(shown: Shown, marked: string[], back: string[], opened: ReadonlySet<string>, nest: Nest): [string, Record<string, unknown>] {
  const accounts = accountsOf(shown, nest);
  const page = onScreen(shown, opened, marked, nest);
  const on = [...page.drawn, ...page.folded];
  const named = (refs: string[]) =>
    refs
      .map((r) =>
        about(
          accounts,
          on.find((c) => refOf(c) === r),
          r,
        ),
      )
      .join("; ");
  const selected = marked.length > 0 ? marked : null;
  const marks = selected ? `; selected ${named(marked)}` : "";
  const folded = page.folded.map(refOf);
  const continued = page.continued.map(refOf);
  if (shown.kind === "casts") {
    const casts = shown.casts.map(refOf);
    const state = { view: "casts", casts, selected, back, folded, continued };
    return [`${count(casts.length, "cast")}: ${named(casts)}${marks}`, state];
  }
  if (shown.kind === "profile") {
    const casts = shown.casts.map(refOf);
    const fid = shown.profile.fid;
    const state = { view: "profile", fid, casts, selected, back, folded, continued };
    const latest = casts.length > 0 ? `: ${named(casts)}` : "";
    return [`the profile of ${nameOf(shown.profile)}, ${count(casts.length, "latest cast")}${latest}${marks}`, state];
  }
  const { thread, only } = shown;
  const cast = refOf(thread.cast);
  const text =
    `the thread of ${about(accounts, thread.cast, cast)}, ${count(thread.ancestors.length, "cast")} above it, ` +
    count(thread.replies.length, "reply", "replies") +
    (only ? `, picked first ${named(only)}` : "") +
    (folded.length > 0 ? `; ${count(folded.length, "cast")} folded` : "") +
    (continued.length > 0 ? `; ${count(continued.length, "reply", "replies")} continuing in their own thread` : "") +
    marks;
  const filter = only ? { only } : {};
  const state = { view: "thread", cast, ...filter, selected, back, folded, continued, depth: nest.depth };
  return [text, state];
}
