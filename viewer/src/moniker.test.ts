import { describe, expect, test } from "bun:test";

import { MAX_LISTED, type Spot, atOf, monikerOf, parseAt } from "./moniker";

const project = "paircaster";
const base = "paircaster://paircaster/";
const a = "3:0x" + "a".repeat(40);
const b = "194:0x" + "b".repeat(40);
const c = "2:0x" + "C".repeat(40);

describe("parseAt", () => {
  test("a bare cast is its whole thread", () => {
    expect(parseAt(project, a)).toEqual({ spot: { kind: "thread", cast: a, only: null }, marks: [] });
  });

  test("a thread, with only some replies and marks", () => {
    expect(parseAt(project, `${base}thread/${a}?only=${b},${c}#cast=${c}`)).toEqual({
      spot: { kind: "thread", cast: a, only: [b, c] },
      marks: [c],
    });
  });

  test("a list of casts, in its order", () => {
    expect(parseAt(project, `${base}casts/${b},${a}#cast=${a}`)).toEqual({
      spot: { kind: "casts", casts: [b, a] },
      marks: [a],
    });
  });

  test("refuses another project, a query on a list, a bad cast and too many casts", () => {
    expect(() => parseAt(project, `paircaster://other/thread/${a}`)).toThrow("not a cast or a moniker of this project");
    expect(() => parseAt(project, `${base}casts/${a}?only=${b}`)).toThrow("a list of casts takes no ?only=");
    expect(() => parseAt(project, `${base}thread/${a}?only=3:0xabc`)).toThrow('not a cast (FID:HASH): "3:0xabc"');
    expect(() => parseAt(project, `${base}thread/${a}#pick=${b}`)).toThrow("expected cast=<cast>,…");
    expect(() => parseAt(project, `${base}profile/3`)).toThrow("not a thread or a list of casts");
    const many = Array.from({ length: MAX_LISTED + 1 }, () => a).join(",");
    expect(() => parseAt(project, `${base}casts/${many}`)).toThrow(`more than ${MAX_LISTED} casts`);
  });
});

describe("monikerOf", () => {
  const spots: Spot[] = [
    { kind: "thread", cast: a, only: null },
    { kind: "thread", cast: a, only: [c, b] },
    { kind: "casts", casts: [b, a, c] },
  ];

  test("names each spot so that parseAt reads it back", () => {
    for (const spot of spots) expect(parseAt(project, monikerOf(project, spot))).toEqual({ spot, marks: [] });
  });

  test("the forms the agent sees", () => {
    expect(monikerOf(project, spots[1])).toBe(`${base}thread/${a}?only=${c},${b}`);
    expect(monikerOf(project, spots[2])).toBe(`${base}casts/${b},${a},${c}`);
  });

  test("a whole thread is kept as its cast, anything else as a moniker", () => {
    expect(atOf(project, spots[0])).toBe(a);
    expect(atOf(project, spots[1])).toBe(monikerOf(project, spots[1]));
  });
});
