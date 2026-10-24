import { describe, expect, test } from "bun:test";

import { GAP, MARGIN, countLabel, place, profileJob, shownAccount, wanted } from "./author";
import type { Profile } from "./reads";

const profile = (fid: number, more: Partial<Profile> = {}): Profile => ({
  version: 1,
  fid,
  username: "alice",
  display_name: "Alice",
  bio: "builds things",
  pfp_url: "https://example.com/a.png",
  url: null,
  location: null,
  twitter: null,
  github: null,
  banner_url: null,
  following: 10,
  followers: 25,
  followers_exact: true,
  ...more,
});

describe("countLabel", () => {
  test("in full below ten thousand", () => {
    expect(countLabel(0)).toBe("0");
    expect(countLabel(999)).toBe("999");
    expect(countLabel(9_999)).toBe("9,999");
  });
  test("to three figures above, rounded down", () => {
    expect(countLabel(10_000)).toBe("10K");
    expect(countLabel(12_399)).toBe("12.3K");
    expect(countLabel(999_999)).toBe("999K");
    expect(countLabel(1_250_000)).toBe("1.2M");
  });
  test("a count that stopped short ends in a plus", () => {
    expect(countLabel(10_000, false)).toBe("10K+");
    expect(countLabel(420, false)).toBe("420+");
  });
});

describe("wanted", () => {
  test("reads an account the page holds nothing of", () => {
    expect(wanted(new Map(), new Set(), 3)).toBe(true);
  });
  test("not one read, refused, or already asked for", () => {
    expect(wanted(new Map([[3, profile(3)]]), new Set(), 3)).toBe(false);
    expect(wanted(new Map([[3, null]]), new Set(), 3)).toBe(false);
    expect(wanted(new Map(), new Set([profileJob(3)]), 3)).toBe(false);
  });
  test("another account's read is not this one's", () => {
    expect(wanted(new Map([[4, profile(4)]]), new Set([profileJob(4)]), 3)).toBe(true);
  });
});

describe("shownAccount", () => {
  const held = { fid: 3, username: "old", display_name: null, pfp_url: null };
  test("the profile once read", () => {
    expect(shownAccount(3, held, profile(3))).toEqual({
      fid: 3,
      username: "alice",
      display_name: "Alice",
      pfp_url: "https://example.com/a.png",
    });
  });
  test("what the page holds before, or when the read failed", () => {
    expect(shownAccount(3, held, undefined)).toBe(held);
    expect(shownAccount(3, held, null)).toBe(held);
    expect(shownAccount(3, undefined, null)).toEqual({ fid: 3, username: null, display_name: null, pfp_url: null });
  });
});

describe("place", () => {
  const pane = { width: 400, height: 800 };
  const card = { width: 300, height: 150 };
  const at = (top: number, left: number, height = 20, width = 60) => ({
    top,
    left,
    bottom: top + height,
    right: left + width,
  });

  test("below the anchor, at its left edge", () => {
    expect(place(at(100, 50), card, pane)).toEqual({ top: 120 + GAP, left: 50, above: false });
  });
  test("above when it fits only there", () => {
    expect(place(at(700, 50), card, pane)).toEqual({ top: 700 - GAP - 150, left: 50, above: true });
  });
  test("moved in from the right edge of a narrow pane", () => {
    expect(place(at(100, 250), card, pane).left).toBe(400 - MARGIN - 300);
  });
  test("a card wider than the pane starts at its left edge", () => {
    expect(place(at(100, 20), { width: 500, height: 150 }, pane).left).toBe(MARGIN);
  });
  test("fitting neither way, on the roomier side, inside the pane", () => {
    const low = place(at(150, 0), card, { width: 400, height: 200 });
    expect(low).toEqual({ top: MARGIN, left: MARGIN, above: true });
    const high = place(at(20, 0), card, { width: 400, height: 190 });
    expect(high).toEqual({ top: 190 - MARGIN - 150, left: MARGIN, above: false });
  });
});
