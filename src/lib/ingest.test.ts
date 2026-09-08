import { describe, expect, it } from "vitest";
import { looksLikePlaylist, splitInputs } from "./ingest";

describe("splitInputs", () => {
  it("splits lines and commas", () => {
    expect(splitInputs("a\nb, c")).toEqual(["a", "b", "c"]);
  });

  it("ignores blank rows", () => {
    expect(splitInputs("  \nhttps://youtu.be/x  \n")).toEqual(["https://youtu.be/x"]);
  });
});

describe("looksLikePlaylist", () => {
  it("treats watch URLs as singles even with list=", () => {
    expect(
      looksLikePlaylist("https://music.youtube.com/watch?v=abc&list=PLxxxx")
    ).toBe(false);
  });

  it("detects youtube and spotify collections", () => {
    expect(looksLikePlaylist("https://music.youtube.com/playlist?list=PLxxxx")).toBe(true);
    expect(looksLikePlaylist("https://open.spotify.com/album/123")).toBe(true);
  });
});
