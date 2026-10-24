// Monikers: what `paircaster viewer open` takes and `viewer state` reports.
// The form is documented for the agent in skill/SKILL.md; it only grows.

// The most casts one moniker may name.
export const MAX_LISTED = 50;

const SCHEME = "paircaster";
const CAST = /^\d+:0x[0-9a-f]{40}$/i;

// What a moniker names, after `paircaster://<project>/`: `thread/<cast>`, with
// `?only=<reply>,…` to show only those replies, in that order; or
// `casts/<cast>,…`, casts from anywhere, in that order. Either may end
// `#cast=<cast>,…` to mark casts it shows.
export type Spot = { kind: "thread"; cast: string; only: string[] | null } | { kind: "casts"; casts: string[] };

// Where monikers of the host's project start.
export const baseOf = (project: string) => `${SCHEME}://${project}/`;

export function monikerOf(project: string, spot: Spot): string {
  const base = baseOf(project);
  if (spot.kind === "casts") return `${base}casts/${spot.casts.join(",")}`;
  return `${base}thread/${spot.cast}${spot.only ? `?only=${spot.only.join(",")}` : ""}`;
}

// A whole thread by its cast, the form `paircaster viewer open` takes most.
export const atOf = (project: string, spot: Spot) =>
  spot.kind === "thread" && !spot.only ? spot.cast : monikerOf(project, spot);

// A cast or a moniker, into what it shows and marks; throws on anything else.
export function parseAt(project: string, at: string): { spot: Spot; marks: string[] } {
  if (CAST.test(at)) return { spot: { kind: "thread", cast: at, only: null }, marks: [] };
  const base = baseOf(project);
  if (!at.startsWith(base)) throw new Error(`not a cast or a moniker of this project: ${at}`);
  const [body, fragment] = cut(at.slice(base.length), "#");
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
