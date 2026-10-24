import { describe, expect, test } from "bun:test";

import { BRANCH_READ, COUNTED, repliesLabel, seedOf } from "./branch";
import { FIRST_REPLIES, type Layout, type Piece, drawnOf, foldedOf, layout } from "./fold";
import type { Shown } from "./shown";
import { describe as tellState } from "./state";
import { type Cast, type Thread, refOf } from "./thread";

const cast = (fid: number, n: number): Cast => ({
  hash: "0x" + n.toString(16).padStart(40, "0"),
  fid,
  timestamp: "2026-10-20T12:00:00Z",
  text: `cast ${n}`,
  display_text: null,
  mentions: [],
  embeds: [],
  parent: null,
  signer: "0x",
  app_fid: 9152,
});

const casts = (from: number, n: number) => Array.from({ length: n }, (_, i) => cast(2 + (i % 3), from + i));

const thread = (above: number, replies: number): Thread => ({
  version: 1,
  top: "root",
  ancestors: casts(100, above),
  cast: cast(3, 1),
  replies: casts(200, replies),
  accounts: [],
});

const refs = (l: Cast[]) => l.map(refOf);
// Each piece of a layout as a cast's number, or "+n" for a fold of n.
const shape = (l: Layout) => {
  const of = (p: Piece) => ("cast" in p ? parseInt(p.cast.hash, 16) : `+${p.casts.length}`);
  return [...l.above.map(of), "|", ...l.replies.map(of)];
};

const none = new Set<string>();

describe("layout", () => {
  test("a short thread folds nothing", () => {
    const t = thread(3, FIRST_REPLIES + 1);
    expect(foldedOf(layout(t, null, none, []))).toEqual([]);
    expect(drawnOf(layout(t, null, none, []))).toEqual([...t.ancestors, ...t.replies]);
  });

  test("keeps the root and the cast replied to, and folds the casts between", () => {
    expect(shape(layout(thread(5, 0), null, none, []))).toEqual([100, "+3", 104, "|"]);
  });

  test("folds the replies after the first few", () => {
    const l = layout(thread(0, FIRST_REPLIES + 8), null, none, []);
    expect(shape(l).slice(-2)).toEqual([200 + FIRST_REPLIES - 1, "+8"]);
  });

  test("an opened fold shows its casts in place", () => {
    const t = thread(5, 20);
    const l = layout(t, null, new Set(["above", "replies"]), []);
    expect(foldedOf(l)).toEqual([]);
    expect(drawnOf(l)).toEqual([...t.ancestors, ...t.replies]);
  });

  test("a run holding a marked cast is not folded", () => {
    const t = thread(5, 20);
    const l = layout(t, null, none, [refOf(t.ancestors[2]), refOf(t.replies[15])]);
    expect(foldedOf(l)).toEqual([]);
  });

  test("the agent's picks come first, in its order, and the other replies fold below", () => {
    const t = thread(0, 5);
    const only = [refOf(t.replies[3]), refOf(t.replies[1])];
    const l = layout(t, only, none, []);
    expect(shape(l)).toEqual(["|", 203, 201, "+3"]);
    expect(refs(foldedOf(l))).toEqual(refs([t.replies[0], t.replies[2], t.replies[4]]));
    expect(refs(drawnOf(layout(t, only, new Set(["replies"]), [])))).toEqual(
      refs([t.replies[3], t.replies[1], t.replies[0], t.replies[2], t.replies[4]]),
    );
  });
});

describe("viewer state", () => {
  test("lists what is folded, and nothing once opened", () => {
    const t = thread(5, 12);
    const shown: Shown = { kind: "thread", thread: t, only: null };
    const [text, state] = tellState(shown, [], [], none);
    expect(state.folded).toEqual(refs([...t.ancestors.slice(1, 4), ...t.replies.slice(FIRST_REPLIES)]));
    expect(text).toContain("5 casts folded");
    expect(tellState(shown, [], [], new Set(["above", "replies"]))[1].folded).toEqual([]);
  });

  test("a list folds nothing", () => {
    const shown: Shown = { kind: "casts", casts: casts(1, 3), accounts: [] };
    expect(tellState(shown, [], [], none)[1].folded).toEqual([]);
  });
});

describe("reply counts", () => {
  const branch = (n: number) => ({ casts: casts(1, n), accounts: [] });

  test("says how many, nothing for none, and past the count only that there are more", () => {
    expect(repliesLabel(branch(0))).toBeNull();
    expect(repliesLabel(branch(1))).toBe("1 reply");
    expect(repliesLabel(branch(COUNTED))).toBe(`${COUNTED} replies`);
    expect(repliesLabel(branch(BRANCH_READ))).toBe(`${COUNTED}+ replies`);
  });

  test("the thread read counts the focus only when it holds all of its replies", () => {
    expect(seedOf(thread(0, 99))?.casts.length).toBe(99);
    expect(seedOf(thread(0, 100))).toBeNull();
  });
});
