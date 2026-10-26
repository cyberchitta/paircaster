import { describe, expect, test } from "bun:test";

import { BRANCH_READ, type Branch, COUNTED, repliesLabel, seedOf } from "./branch";
import { continuedOf, DEPTH, drawnOf, FIRST_REPLIES, foldedOf, type Layout, layout, type Nest, type Piece, repliesFold, SHOWN } from "./fold";
import type { Paircaster } from "./runtime";
import { reach, type Shown } from "./shown";
import { describe as tellState } from "./state";
import { type Cast, refOf, type Thread } from "./thread";

// A map's value at `key`, failing the test where it has none.
function got<V>(map: ReadonlyMap<string, V>, key: string): V {
  const value = map.get(key);
  if (value === undefined) throw new Error(`nothing at ${key}`);
  return value;
}
const below = (branches: ReadonlyMap<string, Branch>, c: Cast) => got(branches, refOf(c)).casts;

const cast = (fid: number, n: number): Cast => ({
  hash: `0x${n.toString(16).padStart(40, "0")}`,
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

const refs = (l: readonly Cast[]) => l.map(refOf);
// Each piece of a layout as a cast's number, or "+n" for a fold of n.
const shape = (l: Layout) => {
  const of = (p: Piece) => ("cast" in p ? parseInt(p.cast.hash, 16) : `+${p.casts.length}`);
  return [...l.above.map(of), "|", ...l.replies.map(of)];
};

const none = new Set<string>();
// Nothing read below the focus.
const flat: Nest = { branches: new Map(), depth: DEPTH };

describe("layout", () => {
  test("a short thread folds nothing", () => {
    const t = thread(3, FIRST_REPLIES + 1);
    expect(foldedOf(layout(t, null, none, [], flat))).toEqual([]);
    expect(drawnOf(layout(t, null, none, [], flat))).toEqual([...t.ancestors, ...t.replies]);
  });

  test("keeps the root and the cast replied to, and folds the casts between", () => {
    expect(shape(layout(thread(5, 0), null, none, [], flat))).toEqual([100, "+3", 104, "|"]);
  });

  test("folds the replies after the first few", () => {
    const l = layout(thread(0, FIRST_REPLIES + 8), null, none, [], flat);
    expect(shape(l).slice(-2)).toEqual([200 + FIRST_REPLIES - 1, "+8"]);
  });

  test("an opened fold shows its casts in place", () => {
    const t = thread(5, 20);
    const l = layout(t, null, new Set(["above", "replies"]), [], flat);
    expect(foldedOf(l)).toEqual([]);
    expect(drawnOf(l)).toEqual([...t.ancestors, ...t.replies]);
  });

  test("a run holding a marked cast is not folded", () => {
    const t = thread(5, 20);
    const l = layout(t, null, none, [refOf(t.ancestors[2]), refOf(t.replies[15])], flat);
    expect(foldedOf(l)).toEqual([]);
  });

  test("the agent's picks come first, in its order, and the other replies fold below", () => {
    const t = thread(0, 5);
    const only = [refOf(t.replies[3]), refOf(t.replies[1])];
    const l = layout(t, only, none, [], flat);
    expect(shape(l)).toEqual(["|", 203, 201, "+3"]);
    expect(refs(foldedOf(l))).toEqual(refs([t.replies[0], t.replies[2], t.replies[4]]));
    expect(refs(drawnOf(layout(t, only, new Set(["replies"]), [], flat)))).toEqual(
      refs([t.replies[3], t.replies[1], t.replies[0], t.replies[2], t.replies[4]]),
    );
  });
});

// A thread whose focus has `top` replies (200…), each reply `i` of the
// first having `fan` replies of its own, and so on `levels` deep, numbered
// from 1000 * level.
function tree(top: number, fan: number, levels: number) {
  const t = thread(0, top);
  const branches = new Map<string, Branch>();
  let parents = [t.replies[0]];
  for (let level = 2; level <= levels; level++) {
    const next: Cast[] = [];
    parents.forEach((p, i) => {
      const kids = casts(1000 * level + 100 * i, fan);
      branches.set(refOf(p), { casts: kids, accounts: [] });
      next.push(...kids);
    });
    parents = next;
  }
  return { t, branches };
}

// Each piece as a cast's number, with its replies in brackets, ">" for a
// thread that continues, or "+n" for a fold of n.
const nested = (ps: Piece[]): unknown[] =>
  ps.map((p) =>
    "fold" in p
      ? `+${p.casts.length}`
      : p.replies.length > 0
        ? [parseInt(p.cast.hash, 16), nested(p.replies)]
        : p.continues
          ? `${parseInt(p.cast.hash, 16)}>`
          : parseInt(p.cast.hash, 16),
  );

describe("nested replies", () => {
  test("nest under their reply, a few shown at each level and the rest folded", () => {
    const { t, branches } = tree(1, 5, 3);
    const l = layout(t, null, none, [], { branches, depth: DEPTH });
    expect(SHOWN.slice(1)).toEqual([3, 1]);
    expect(nested(l.replies)).toEqual([[200, [[2000, [3000, "+4"]], [2001, [3100, "+4"]], [2002, [3200, "+4"]], "+2"]]]);
  });

  test("past the depth the page nests, a reply with replies continues in its own thread", () => {
    const { t, branches } = tree(1, 1, 4);
    expect(nested(layout(t, null, none, [], { branches, depth: 3 }).replies)).toEqual([[200, [[2000, ["3000>"]]]]]);
    const narrow = layout(t, null, none, [], { branches, depth: 2 });
    expect(nested(narrow.replies)).toEqual([[200, ["2000>"]]]);
    expect(refs(continuedOf(narrow))).toEqual([refOf(below(branches, t.replies[0])[0])]);
  });

  test("an opened nested fold shows its casts, and their replies are counted as drawn", () => {
    const { t, branches } = tree(1, 5, 2);
    const fold = repliesFold(refOf(t.replies[0]));
    const l = layout(t, null, new Set([fold]), [], { branches, depth: DEPTH });
    expect(foldedOf(l)).toEqual([]);
    expect(drawnOf(l)).toEqual([t.replies[0], ...below(branches, t.replies[0])]);
  });

  test("the way down to a marked cast is drawn, unfolded and past the depth", () => {
    const { t, branches } = tree(1, 3, 4);
    const deep = got(branches, refOf(below(branches, below(branches, t.replies[0])[2])[1]));
    const mark = deep.casts[2];
    const l = layout(t, null, none, [refOf(mark)], { branches, depth: 2 });
    expect(drawnOf(l).map((c) => parseInt(c.hash, 16))).toContain(parseInt(mark.hash, 16));
    expect(nested(l.replies)).toEqual([[200, ["2000>", "2001>", [2002, ["3200>", [3201, [4700, 4701, 4702]], "3202>"]]]]]);
  });
});

describe("a marked nested reply", () => {
  // A node holding `t` and `branches`: a cast's thread names the casts above
  // it, and its replies are its branch; it counts the reads it answers.
  function node(t: Thread, branches: Map<string, Branch>) {
    const parent = new Map<string, Cast>(t.replies.map((c) => [refOf(c), t.cast]));
    const byRef = new Map<string, Cast>([[refOf(t.cast), t.cast], ...t.replies.map((c) => [refOf(c), c] as const)]);
    for (const [ref, b] of branches) {
      for (const c of b.casts) {
        parent.set(refOf(c), got(byRef, ref));
        byRef.set(refOf(c), c);
      }
    }
    const reads: string[] = [];
    const rt = {
      thread: async (ref: string) => {
        const ancestors: Cast[] = [];
        for (let p = parent.get(ref); p; p = parent.get(refOf(p))) ancestors.unshift(p);
        return { ...t, ancestors, cast: got(byRef, ref), replies: [] };
      },
      replies: async (ref: string) => {
        reads.push(ref);
        return { version: 1, parent: { fid: 0, hash: "" }, ...got(branches, ref) };
      },
    } as unknown as Paircaster;
    return { rt, reads };
  }

  test("is reached by reading each reply on the way down to it", async () => {
    const { t, branches } = tree(1, 2, 4);
    const { rt, reads } = node(t, branches);
    const l2 = below(branches, t.replies[0])[1];
    const l3 = below(branches, l2)[0];
    const mark = below(branches, l3)[1];
    const known = new Map([[refOf(t.replies[0]), got(branches, refOf(t.replies[0]))]]);
    const way = await reach(rt, t, refOf(mark), known);
    expect([...way.keys()]).toEqual(refs([t.replies[0], l2, l3]));
    expect(reads).toEqual(refs([l2, l3]));
  });

  test("is refused when it is not under the focus", async () => {
    const { t, branches } = tree(2, 2, 3);
    const { rt } = node(t, branches);
    const elsewhere = { ...t, cast: t.replies[1], replies: [] };
    const mark = below(branches, t.replies[0])[0];
    await expect(reach(rt, elsewhere, refOf(mark), new Map())).rejects.toThrow("is not on the page");
  });
});

describe("viewer state", () => {
  test("lists what is folded, and nothing once opened", () => {
    const t = thread(5, 12);
    const shown: Shown = { kind: "thread", thread: t, only: null };
    const [text, state] = tellState(shown, [], [], none, flat);
    expect(state.folded).toEqual(refs([...t.ancestors.slice(1, 4), ...t.replies.slice(FIRST_REPLIES)]));
    expect(text).toContain("5 casts folded");
    expect(tellState(shown, [], [], new Set(["above", "replies"]), flat)[1].folded).toEqual([]);
  });

  test("lists nested folds, the replies that continue, and how deep the page nests", () => {
    const { t, branches } = tree(1, 5, 4);
    const shown: Shown = { kind: "thread", thread: t, only: null };
    const nest = { branches, depth: 2 };
    const level2 = below(branches, t.replies[0]);
    const [text, state] = tellState(shown, [], [], none, nest);
    expect(state.folded).toEqual(refs(level2.slice(3)));
    expect(state.continued).toEqual(refs(level2.slice(0, 3)));
    expect(state.depth).toBe(2);
    expect(text).toContain("3 replies continuing in their own thread");
  });

  test("names a selected nested reply", () => {
    const { t, branches } = tree(1, 2, 2);
    const shown: Shown = { kind: "thread", thread: t, only: null };
    const reply = below(branches, t.replies[0])[1];
    const [text, state] = tellState(shown, [refOf(reply)], [], none, { branches, depth: DEPTH });
    expect(state.selected).toEqual([refOf(reply)]);
    expect(text).toContain(`selected ${refOf(reply)} by fid ${reply.fid} ("cast ${parseInt(reply.hash, 16)}")`);
  });

  test("a list folds nothing", () => {
    const shown: Shown = { kind: "casts", casts: casts(1, 3), accounts: [], parents: new Map() };
    expect(tellState(shown, [], [], none, flat)[1].folded).toEqual([]);
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
