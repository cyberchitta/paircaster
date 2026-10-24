import { describe, expect, test } from "bun:test";

import { altOf, drawable, hostOf, isImage } from "./media";

describe("images", () => {
  test("are Farcaster's image host or an image file", () => {
    expect(isImage("https://imagedelivery.net/abc/def/original")).toBe(true);
    expect(isImage("https://example.com/a/cat.JPG")).toBe(true);
    expect(isImage("https://example.com/a/cat.html")).toBe(false);
    expect(isImage("not a url")).toBe(false);
  });

  test("are drawn only from https", () => {
    expect(drawable("https://i.imgur.com/x.png")).toBe(true);
    expect(drawable("http://i.imgur.com/x.png")).toBe(false);
    expect(drawable("https://example.com/")).toBe(false);
  });

  test("are named by their host", () => {
    expect(hostOf("https://www.example.com/x.png")).toBe("example.com");
    expect(altOf("https://i.imgur.com/x.png")).toBe("image from i.imgur.com");
  });
});
