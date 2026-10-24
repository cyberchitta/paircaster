import { describe, expect, test } from "bun:test";

import { spanned, toggled } from "./marks";
import { MAX_LISTED } from "./moniker";

const order = ["a", "b", "c", "d", "e"];

describe("toggled", () => {
  test("adds a cast at the end, and takes out one marked", () => {
    expect(toggled(["b"], "d")).toEqual(["b", "d"]);
    expect(toggled(["b", "d"], "b")).toEqual(["d"]);
    expect(toggled([], "a")).toEqual(["a"]);
  });

  test("adds nothing past what a moniker holds", () => {
    const full = Array.from({ length: MAX_LISTED }, (_, i) => `x${i}`);
    expect(toggled(full, "y")).toEqual(full);
  });
});

describe("spanned", () => {
  test("marks every cast between the anchor and the cast, either way up", () => {
    expect(spanned(["b"], order, "b", "d")).toEqual(["b", "c", "d"]);
    expect(spanned(["d"], order, "d", "b")).toEqual(["d", "b", "c"]);
  });

  test("keeps what was marked, and adds each cast once", () => {
    expect(spanned(["e", "c"], order, "c", "a")).toEqual(["e", "c", "a", "b"]);
  });

  test("marks only the cast with no anchor on the page", () => {
    expect(spanned([], order, null, "c")).toEqual(["c"]);
    expect(spanned(["a"], order, "gone", "c")).toEqual(["a", "c"]);
  });
});
