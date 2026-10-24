// Monikers: what `paircaster viewer open` takes and `viewer state` reports.
// The form is documented for the agent in skill/SKILL.md; it only grows.

// The most casts one moniker may name.
export const MAX_LISTED = 50;

const SCHEME = "paircaster";
const CAST = /^\d+:0x[0-9a-f]{40}$/i;

// What a moniker names, after `paircaster://<project>/`: `thread/<cast>`, with
// `?only=<reply>,…` to show only those replies, in that order; or
// `casts/<cast>,…`, casts from anywhere, in that order. Either may end
// `#cast=<cast>,…` to mark casts it shows. Any may carry
// `?caption=<text>`, URL-encoded: a line the agent shows above the view.
export type Spot = ({ kind: "thread"; cast: string; only: string[] | null } | { kind: "casts"; casts: string[] }) & {
  caption?: string;
};

// The longest caption, in characters.
export const MAX_CAPTION = 200;

// Where monikers of the host's project start.
export const baseOf = (project: string) => `${SCHEME}://${project}/`;

export function monikerOf(project: string, spot: Spot): string {
  const only = spot.kind === "thread" && spot.only ? [`only=${spot.only.join(",")}`] : [];
  const caption = spot.caption !== undefined ? [`caption=${encodeURIComponent(spot.caption)}`] : [];
  const query = [...only, ...caption];
  return `${baseOf(project)}${pathOf(spot)}${query.length > 0 ? `?${query.join("&")}` : ""}`;
}

const pathOf = (spot: Spot) => (spot.kind === "casts" ? `casts/${spot.casts.join(",")}` : `thread/${spot.cast}`);

// The view `spot` with `marks` marked, as `#cast=` names them; null for none.
export const markedOf = (project: string, spot: Spot, marks: string[]): string | null =>
  marks.length > 0 ? `${monikerOf(project, spot)}#cast=${marks.join(",")}` : null;

// A whole thread by its cast, the form `paircaster viewer open` takes most.
export const atOf = (project: string, spot: Spot) =>
  spot.kind === "thread" && !spot.only && spot.caption === undefined ? spot.cast : monikerOf(project, spot);

// A cast or a moniker, into what it shows and marks; throws on anything else.
export function parseAt(project: string, at: string): { spot: Spot; marks: string[] } {
  if (CAST.test(at)) return { spot: { kind: "thread", cast: at, only: null }, marks: [] };
  const base = baseOf(project);
  if (!at.startsWith(base)) throw new Error(`not a cast or a moniker of this project: ${at}`);
  const [body, fragment] = cut(at.slice(base.length), "#");
  const [path, query] = cut(body, "?");
  const marks = fragment === null ? [] : listAfter(fragment, "cast=", at);
  const params = paramsOf(query, at);
  const caption = params.has("caption") ? captionOf(params.get("caption")!, at) : undefined;
  const captioned = caption === undefined ? {} : { caption };
  if (path.startsWith("casts/")) {
    const other = [...params.keys()].find((k) => k !== "caption");
    if (other !== undefined) throw new Error(`a list of casts takes no ?${other}=: ${at}`);
    return { spot: { kind: "casts", casts: list(path.slice(6), at), ...captioned }, marks };
  }
  const cast = path.startsWith("thread/") ? path.slice(7) : "";
  if (!CAST.test(cast)) throw new Error(`not a thread or a list of casts: ${at}`);
  const other = [...params.keys()].find((k) => k !== "caption" && k !== "only");
  if (other !== undefined) throw new Error(`a thread takes ?only= and ?caption=, not ?${other}=: ${at}`);
  const only = params.has("only") ? list(params.get("only")!, at) : null;
  return { spot: { kind: "thread", cast, only, ...captioned }, marks };
}

// A query's `key=value` pairs, joined by `&`, each key once.
function paramsOf(query: string | null, at: string): Map<string, string> {
  const params = new Map<string, string>();
  for (const pair of query === null ? [] : query.split("&")) {
    const [key, value] = cut(pair, "=");
    if (value === null) throw new Error(`expected key=value, not "${pair}", in ${at}`);
    if (params.has(key)) throw new Error(`?${key}= given twice in ${at}`);
    params.set(key, value);
  }
  return params;
}

// The agent's caption, URL-decoded: one line of text, never markup.
function captionOf(encoded: string, at: string): string {
  let text: string;
  try {
    text = decodeURIComponent(encoded);
  } catch {
    throw new Error(`a caption is URL-encoded (%20 for a space): ${at}`);
  }
  if (text.trim() === "") throw new Error(`an empty caption in ${at}`);
  if (/\p{Cc}/u.test(text)) throw new Error(`a caption is one line, with no control characters: ${at}`);
  if (Array.from(text).length > MAX_CAPTION) throw new Error(`a caption longer than ${MAX_CAPTION} characters in ${at}`);
  return text;
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
