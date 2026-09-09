import { describe, expect, it } from "vitest";
import {
  filterTracks,
  parseEtaSeconds,
  pruneDismissedJobErrors,
  sortTracks,
  stageIndex,
  standaloneTracks,
  tracksInPlaylist,
  visibleFailedTracks,
} from "./library";
import type { Track } from "./types";

function track(partial: Partial<Track>): Track {
  return {
    id: "1",
    title: "B",
    artist: "Zed",
    sourceKind: "youtube",
    status: "ready",
    createdAt: 10,
    updatedAt: 10,
    durationMs: 1000,
    ...partial,
  };
}

describe("library helpers", () => {
  it("sorts by title and recency", () => {
    const tracks = [
      track({ id: "a", title: "Beta", createdAt: 1 }),
      track({ id: "b", title: "Alpha", createdAt: 5 }),
    ];
    expect(sortTracks(tracks, "title").map((item) => item.title)).toEqual(["Alpha", "Beta"]);
    expect(sortTracks(tracks, "recent")[0].id).toBe("b");
  });

  it("filters by artist or title", () => {
    const tracks = [track({ title: "Neon", artist: "Pulse" }), track({ id: "2", title: "Quiet" })];
    expect(filterTracks(tracks, "pul").map((item) => item.title)).toEqual(["Neon"]);
  });

  it("parses eta from payload or message", () => {
    expect(parseEtaSeconds({ etaSeconds: 12, message: "" })).toBe(12);
    expect(parseEtaSeconds({ message: "Separating chunk 2/9 · ETA 44s" })).toBe(44);
  });

  it("maps separate stage onto infer", () => {
    expect(stageIndex("separate")).toBe(2);
    expect(stageIndex("download")).toBe(0);
  });

  it("groups playlist tracks separately from standalone ones", () => {
    const tracks = [
      track({ id: "a", title: "Single" }),
      track({ id: "b", title: "Inside", playlistId: "pl", playlistIndex: 1 }),
      track({ id: "c", title: "First", playlistId: "pl", playlistIndex: 0 }),
    ];
    expect(standaloneTracks(tracks).map((item) => item.id)).toEqual(["a"]);
    expect(tracksInPlaylist(tracks, "pl").map((item) => item.title)).toEqual(["First", "Inside"]);
  });

  it("hides dismissed failed tracks and prunes stale dismiss ids", () => {
    const tracks = [
      track({ id: "fail", status: "error", error: "nope" }),
      track({ id: "ok", status: "ready" }),
    ];
    expect(visibleFailedTracks(tracks, ["fail"]).map((item) => item.id)).toEqual([]);
    expect(visibleFailedTracks(tracks, []).map((item) => item.id)).toEqual(["fail"]);
    expect(pruneDismissedJobErrors(tracks, ["fail", "missing", "ok"])).toEqual(["fail"]);
  });
});
