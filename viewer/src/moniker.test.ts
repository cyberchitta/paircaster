import { describe, expect, test } from "bun:test";

import { MAX_CAPTION, MAX_LISTED, type Spot, atOf, markedOf, monikerOf, parseAt } from "./moniker";

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
    expect(() => parseAt(project, `${base}people/3`)).toThrow("not a thread, a list of casts or a profile");
    expect(() => parseAt(project, `${base}profile/dwr`)).toThrow('not a FID: "dwr"');
    expect(() => parseAt(project, `${base}profile/0`)).toThrow('not a FID: "0"');
    expect(() => parseAt(project, `${base}profile/3?only=${a}`)).toThrow("a profile takes no ?only=");
    const many = Array.from({ length: MAX_LISTED + 1 }, () => a).join(",");
    expect(() => parseAt(project, `${base}casts/${many}`)).toThrow(`more than ${MAX_LISTED} casts`);
  });
});

describe("monikerOf", () => {
  const spots: Spot[] = [
    { kind: "thread", cast: a, only: null },
    { kind: "thread", cast: a, only: [c, b] },
    { kind: "casts", casts: [b, a, c] },
    { kind: "thread", cast: a, only: [b], caption: "ranked by likes & recasts, #1 first?" },
    { kind: "profile", fid: 3 },
    { kind: "profile", fid: 3356360, caption: "who replied most" },
    { kind: "casts", casts: [a], caption: 'casts saying "snapchain" — 100% of them' },
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

describe("markedOf", () => {
  test("names the view with the casts marked, or nothing for none", () => {
    const spot: Spot = { kind: "thread", cast: a, only: null };
    expect(markedOf(project, spot, [b, c])).toBe(`${base}thread/${a}#cast=${b},${c}`);
    expect(parseAt(project, markedOf(project, spot, [b, c])!).marks).toEqual([b, c]);
    expect(markedOf(project, spot, [])).toBeNull();
  });
});

describe("captions", () => {
  test("are URL-decoded text, in any order with only", () => {
    expect(parseAt(project, `${base}casts/${a}?caption=most%20liked%20%3Cb%3E`).spot).toEqual({
      kind: "casts",
      casts: [a],
      caption: "most liked <b>",
    });
    expect(parseAt(project, `${base}thread/${a}?caption=top&only=${b}#cast=${b}`)).toEqual({
      spot: { kind: "thread", cast: a, only: [b], caption: "top" },
      marks: [b],
    });
  });

  test("keep a captioned thread a moniker, not a bare cast", () => {
    expect(atOf(project, { kind: "thread", cast: a, only: null, caption: "why" })).toBe(`${base}thread/${a}?caption=why`);
  });

  test("refuses a bad, empty, multi-line or long caption, and other keys", () => {
    expect(() => parseAt(project, `${base}casts/${a}?caption=%E0%A4`)).toThrow("URL-encoded");
    expect(() => parseAt(project, `${base}casts/${a}?caption=%20`)).toThrow("an empty caption");
    expect(() => parseAt(project, `${base}casts/${a}?caption=a%0Ab`)).toThrow("one line");
    const long = "x".repeat(MAX_CAPTION + 1);
    expect(() => parseAt(project, `${base}casts/${a}?caption=${long}`)).toThrow(`longer than ${MAX_CAPTION}`);
    expect(() => parseAt(project, `${base}thread/${a}?caption=a&caption=b`)).toThrow("given twice");
    expect(() => parseAt(project, `${base}thread/${a}?sort=likes`)).toThrow("not ?sort=");
  });
});
