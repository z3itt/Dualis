import type { LibrarySort, Track } from "./types";

export function sortTracks(tracks: Track[], sort: LibrarySort) {
  const copy = [...tracks];
  copy.sort((a, b) => {
    switch (sort) {
      case "title":
        return a.title.localeCompare(b.title);
      case "artist":
        return a.artist.localeCompare(b.artist);
      case "duration":
        return (b.durationMs ?? 0) - (a.durationMs ?? 0);
      case "status":
        return String(a.status).localeCompare(String(b.status));
      default:
        return b.createdAt - a.createdAt;
    }
  });
  return copy;
}

export function filterTracks(tracks: Track[], query: string) {
  const needle = query.trim().toLowerCase();
  if (!needle) {
    return tracks;
  }
  return tracks.filter((track) => `${track.title} ${track.artist}`.toLowerCase().includes(needle));
}

export function standaloneTracks(tracks: Track[]) {
  return tracks.filter((track) => !track.playlistId);
}

export function tracksInPlaylist(tracks: Track[], playlistId: string) {
  return tracks
    .filter((track) => track.playlistId === playlistId)
    .sort((a, b) => (a.playlistIndex ?? 0) - (b.playlistIndex ?? 0));
}

export function parseEtaSeconds(event: { etaSeconds?: number | null; message: string }) {
  if (typeof event.etaSeconds === "number" && Number.isFinite(event.etaSeconds)) {
    return event.etaSeconds;
  }
  const match = event.message.match(/ETA\s+(\d+(?:\.\d+)?)s/i);
  return match ? Number(match[1]) : null;
}

export const STAGES = ["download", "decode", "infer", "export"] as const;

export function stageIndex(stage: string) {
  const normalized = stage === "separate" ? "infer" : stage;
  const index = STAGES.indexOf(normalized as (typeof STAGES)[number]);
  return index < 0 ? 0 : index;
}

export function pruneDismissedJobErrors(tracks: Track[], dismissedIds: string[]) {
  return dismissedIds.filter((id) => tracks.some((track) => track.id === id && track.status === "error"));
}

export function visibleFailedTracks(tracks: Track[], dismissedIds: string[]) {
  const hidden = new Set(dismissedIds);
  return tracks.filter((track) => track.status === "error" && !hidden.has(track.id));
}
