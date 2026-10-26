import { describe, expect, test } from "bun:test";

import { authorOf, excerpt, heldOf, parentOf, parentsOf, quotesOf, wantedOf } from "./context";
import type { Nest } from "./fold";
import { type Shown, contextsOf } from "./shown";
import { type Account, type Cast, type Thread, refOf } from "./thread";

const hash = (n: number) => "0x" + n.toString(16).padStart(40, "0");
const cast = (fid: number, n: number, more: Partial<Cast> = {}): Cast => ({
  hash: hash(n),
  fid,
  timestamp: "2026-10-20T12:00:00Z",
  text: `cast ${n}`,
  display_text: null,
  mentions: [],
  embeds: [],
  parent: null,
  signer: "0x",
  app_fid: 9152,
  ...more,
});
const ref = (fid: number, n: number) => ({ fid, hash: hash(n) });
const replyTo = (fid: number, n: number, to: { fid: number; hash: string }, more: Partial<Cast> = {}) => cast(fid, n, { parent: { cast: to }, ...more });
const quoting = (fid: number, n: number, ...quoted: { fid: number; hash: string }[]) =>
  cast(fid, n, { embeds: [{ url: "https://example.com/a" }, ...quoted.map((q) => ({ cast: q }))] });
const alice: Account = { fid: 2, username: "alice", display_name: "Alice", pfp_url: null };
const flat: Nest = { branches: new Map(), depth: 3 };

describe("which casts a card needs", () => {
  test("a quote's cast embeds, not its links", () => {
    expect(quotesOf(quoting(3, 1, ref(2, 9)))).toEqual(["2:" + hash(9)]);
  });

  test("a reply's parent, but not a channel cast's URL", () => {
    expect(parentOf(replyTo(3, 1, ref(2, 9)))).toBe("2:" + hash(9));
    expect(parentOf(cast(3, 1, { parent: { url: "https://farcaster.xyz/~/channel/x" } }))).toBeNull();
  });

  test("a list wants parents and quotes, a thread quotes only, each once, in page order", () => {
    const drawn = [replyTo(3, 1, ref(2, 9)), quoting(3, 2, ref(4, 8), ref(2, 9)), cast(3, 3)];
    expect(wantedOf(drawn, true)).toEqual(["2:" + hash(9), "4:" + hash(8)]);
    expect(wantedOf(drawn, false)).toEqual(["4:" + hash(8), "2:" + hash(9)]);
  });
});

describe("what the page holds already", () => {
  test("a list cast's parent is the nearest cast its read walked up to", () => {
    const parent = cast(2, 9);
    const t: Thread = {
      version: 1,
      top: "root",
      ancestors: [cast(5, 7), parent],
      cast: replyTo(3, 1, ref(2, 9)),
      replies: [],
      accounts: [alice],
    };
    expect(parentsOf([t]).get(refOf(parent))).toEqual({ cast: parent, account: alice });
  });

  test("a parent the node does not hold is gone; a root has none", () => {
    const missing: Thread = {
      version: 1,
      top: { missing: ref(2, 9) },
      ancestors: [],
      cast: replyTo(3, 1, ref(2, 9)),
      replies: [],
      accounts: [],
    };
    const root: Thread = { ...missing, top: "root", cast: cast(3, 2) };
    expect([...parentsOf([missing, root])]).toEqual([["2:" + hash(9), null]]);
  });

  test("a cast on the page, or read below one, is not read again; over a read, the page's own wins", () => {
    const quoted = cast(2, 9);
    const nested = cast(4, 8);
    const thread: Thread = { version: 1, top: "root", ancestors: [], cast: quoted, replies: [], accounts: [alice] };
    const shown: Shown = { kind: "thread", thread, only: null };
    const nest: Nest = { branches: new Map([[refOf(quoted), { casts: [nested], accounts: [] }]]), depth: 3 };
    const all = contextsOf(
      shown,
      nest,
      new Map([
        [refOf(quoted), null],
        ["7:" + hash(1), null],
      ]),
    );
    expect(all.get(refOf(quoted))).toEqual({ cast: quoted, account: alice });
    expect(all.get(refOf(nested))?.cast).toEqual(nested);
    expect(all.get("7:" + hash(1))).toBeNull();
  });

  test("a list's parents count as held", () => {
    const parent = cast(2, 9);
    const shown: Shown = {
      kind: "casts",
      casts: [replyTo(3, 1, ref(2, 9))],
      accounts: [],
      parents: heldOf([parent], [alice]),
    };
    expect(contextsOf(shown, flat, new Map()).get(refOf(parent))?.account).toEqual(alice);
  });
});

describe("excerpt", () => {
  test("puts a short text on one line", () => {
    expect(excerpt(cast(2, 1, { text: "one\n\ntwo  three " }), 40)).toBe("one two three");
  });

  test("cuts a long text at a word", () => {
    expect(excerpt(cast(2, 1, { text: "alpha beta gamma delta" }), 13)).toBe("alpha beta…");
  });

  test("cuts a text with no space near the end where it must, never inside a character", () => {
    expect(excerpt(cast(2, 1, { text: "🙂".repeat(10) }), 4)).toBe("🙂🙂🙂🙂…");
  });

  test("uses the text with usernames in", () => {
    expect(excerpt(cast(2, 1, { text: "hi @fid:3", display_text: "hi @carol" }), 40)).toBe("hi @carol");
  });
});

test("names a context cast's author by handle, else name, else FID", () => {
  const c = cast(2, 1);
  expect(authorOf({ cast: c, account: alice })).toBe("@alice");
  expect(authorOf({ cast: c, account: { ...alice, username: null } })).toBe("Alice");
  expect(authorOf({ cast: c, account: undefined })).toBe("fid 2");
});
