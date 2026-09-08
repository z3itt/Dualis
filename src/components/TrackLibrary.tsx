import { ChevronLeft, Download, ListPlus, Play, Search, Trash2 } from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";
import { CoverArt } from "@/components/CoverArt";
import { Button } from "@/components/ui/button";
import { deletePlaylist, deleteTrack, deleteTracks, exportStems, exportStemsBatch } from "@/lib/api";
import { filterTracks, sortTracks, standaloneTracks, tracksInPlaylist } from "@/lib/library";
import type { LibrarySort, Track } from "@/lib/types";
import { formatTime } from "@/lib/utils";
import { useAppStore } from "@/store/app";

const SORTS: { id: LibrarySort; label: string }[] = [
  { id: "recent", label: "Recent" },
  { id: "title", label: "Title" },
  { id: "artist", label: "Artist" },
  { id: "duration", label: "Length" },
  { id: "status", label: "Status" },
];

export function TrackLibrary() {
  const tracks = useAppStore((s) => s.tracks);
  const playlists = useAppStore((s) => s.playlists);
  const openPlaylistId = useAppStore((s) => s.openPlaylistId);
  const query = useAppStore((s) => s.query);
  const sort = useAppStore((s) => s.sort);
  const setQuery = useAppStore((s) => s.setQuery);
  const setSort = useAppStore((s) => s.setSort);
  const currentId = useAppStore((s) => s.currentId);
  const selectedIds = useAppStore((s) => s.selectedIds);
  const playTrack = useAppStore((s) => s.playTrack);
  const removeTrack = useAppStore((s) => s.removeTrack);
  const removeTracks = useAppStore((s) => s.removeTracks);
  const removePlaylist = useAppStore((s) => s.removePlaylist);
  const openPlaylist = useAppStore((s) => s.openPlaylist);
  const toggleSelected = useAppStore((s) => s.toggleSelected);
  const clearSelected = useAppStore((s) => s.clearSelected);
  const addToQueue = useAppStore((s) => s.addToQueue);

  const playlist = playlists.find((item) => item.id === openPlaylistId) ?? null;
  const visibleTracks = playlist ? tracksInPlaylist(tracks, playlist.id) : standaloneTracks(tracks);
  const filteredTracks = playlist
    ? filterTracks(visibleTracks, query)
    : sortTracks(filterTracks(visibleTracks, query), sort);
  const filteredPlaylists = playlist
    ? []
    : playlists.filter((item) => `${item.title} ${item.artist}`.toLowerCase().includes(query.trim().toLowerCase()));
  const selectedReady = selectedIds.filter((id) => tracks.find((track) => track.id === id)?.status === "ready");

  return (
    <section className="panel surface flex min-h-0 flex-1 flex-col overflow-hidden p-4 sm:p-5 lg:h-full">
      <div className="mb-4 flex flex-wrap items-end justify-between gap-3">
        <div>
          <div className="mb-2 flex items-center gap-2">
            <span className="flex h-6 w-6 items-center justify-center rounded-full bg-gray-900 text-[11px] font-semibold text-white dark:bg-white dark:text-gray-900">
              2
            </span>
            <span className="rounded-full border border-border px-3 py-1 text-xs font-medium text-muted-foreground">Library</span>
          </div>
          {playlist ? (
            <div className="flex items-center gap-2">
              <Button size="icon" variant="ghost" aria-label="Back to library" onClick={() => openPlaylist(null)}>
                <ChevronLeft className="h-4 w-4" />
              </Button>
              <div>
                <h2 className="text-lg font-medium">{playlist.title}</h2>
                <p className="text-xs text-muted-foreground">
                  {playlist.artist} · {playlist.readyCount}/{playlist.trackCount} ready
                </p>
              </div>
            </div>
          ) : (
            <h2 className="text-lg font-medium">Your library</h2>
          )}
        </div>
        <label className="relative block w-56 max-w-full">
          <span className="sr-only">Search library</span>
          <Search className="pointer-events-none absolute left-3 top-2.5 h-4 w-4 text-muted-foreground" />
          <input
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder={playlist ? "Search playlist" : "Search library"}
            className="h-9 w-full rounded-full border border-border bg-input pl-9 pr-3 text-sm"
          />
        </label>
      </div>
      <div className="mb-3 flex flex-wrap items-center gap-2">
        {playlist ? null : SORTS.map((item) => (
          <button
            key={item.id}
            type="button"
            onClick={() => setSort(item.id)}
            className={`rounded-full px-3 py-1 text-xs transition-colors duration-200 ${
              sort === item.id ? "bg-gray-900 text-white dark:bg-white dark:text-gray-900" : "bg-muted text-muted-foreground hover:text-foreground"
            }`}
          >
            {item.label}
          </button>
        ))}
        <div className="ml-auto flex gap-2">
          {selectedIds.length > 0 ? (
            <>
              <Button
                size="sm"
                variant="outline"
                disabled={selectedReady.length === 0}
                onClick={async () => {
                  const dest = await open({ directory: true });
                  if (typeof dest === "string") {
                    await exportStemsBatch(selectedReady, dest);
                  }
                }}
              >
                <Download className="h-3.5 w-3.5" />
                Export {selectedReady.length}
              </Button>
              <Button
                size="sm"
                variant="danger"
                onClick={async () => {
                  await deleteTracks(selectedIds);
                  removeTracks(selectedIds);
                }}
              >
                <Trash2 className="h-3.5 w-3.5" />
                Delete
              </Button>
              <Button size="sm" variant="ghost" onClick={clearSelected}>
                Clear
              </Button>
            </>
          ) : null}
        </div>
      </div>
      <div className="scroll-thin min-h-0 flex-1 overflow-y-auto">
        {filteredPlaylists.length === 0 && filteredTracks.length === 0 ? (
          <p className="text-sm text-muted-foreground">
            {playlist
              ? "This playlist is empty."
              : "Separated tracks and playlists appear here. Open a playlist to see its songs."}
          </p>
        ) : (
          <ul className="grid grid-cols-1 items-start gap-3 xl:grid-cols-2">
            {filteredPlaylists.map((item) => {
              const members = tracksInPlaylist(tracks, item.id);
              const firstReady = members.find((track) => track.status === "ready");
              const first = firstReady ?? members[0];
              return (
                <li key={item.id} className="group relative flex h-auto items-center gap-3 self-start rounded-xl border border-border bg-muted p-3 transition-colors duration-200 hover:bg-card">
                  <button type="button" className="flex min-w-0 flex-1 items-center gap-3 text-left" onClick={() => openPlaylist(item.id)}>
                    <CoverArt path={item.coverPath} title={item.title} size="md" />
                    <div className="min-w-0">
                      <p className="truncate text-sm font-medium">{item.title}</p>
                      <p className="truncate text-xs text-muted-foreground">
                        {item.artist} · {item.trackCount} tracks · {item.readyCount} ready
                      </p>
                      <span className="mt-1 inline-flex rounded-full bg-orange-100 px-2 py-0.5 text-[10px] uppercase tracking-wide text-orange-800 dark:bg-orange-950 dark:text-orange-200">
                        Playlist
                      </span>
                    </div>
                  </button>
                  <div className="flex items-center gap-1 opacity-100 transition-opacity duration-200 lg:opacity-0 lg:group-hover:opacity-100 lg:group-focus-within:opacity-100">
                    <Button
                      size="icon"
                      variant="ghost"
                      disabled={!first}
                      aria-label={`Play ${item.title}`}
                      onClick={() => first && playTrack(first)}
                    >
                      <Play className="h-4 w-4 fill-current" />
                    </Button>
                    <Button
                      size="icon"
                      variant="ghost"
                      aria-label={`Delete ${item.title}`}
                      onClick={async () => {
                        await deletePlaylist(item.id);
                        removePlaylist(item.id);
                      }}
                    >
                      <Trash2 className="h-4 w-4" />
                    </Button>
                  </div>
                </li>
              );
            })}
            {filteredTracks.map((track) => (
              <TrackRow
                key={track.id}
                track={track}
                active={currentId === track.id}
                checked={selectedIds.includes(track.id)}
                onToggle={() => toggleSelected(track.id)}
                onPlay={() => playTrack(track)}
                onQueue={() => addToQueue(track.id)}
                onDelete={async () => {
                  await deleteTrack(track.id);
                  removeTrack(track.id);
                }}
              />
            ))}
          </ul>
        )}
      </div>
    </section>
  );
}

function TrackRow({
  track,
  active,
  checked,
  onToggle,
  onPlay,
  onQueue,
  onDelete,
}: {
  track: Track;
  active: boolean;
  checked: boolean;
  onToggle: () => void;
  onPlay: () => void;
  onQueue: () => void;
  onDelete: () => void;
}) {
  const ready = track.status === "ready";
  return (
    <li
      className={`group relative flex h-auto items-center gap-3 self-start rounded-xl border p-3 transition-colors duration-200 ${
        active ? "border-primary bg-orange-50 dark:bg-orange-950/20" : "border-border bg-muted hover:bg-card"
      }`}
    >
      <label className="flex h-11 w-11 items-center justify-center">
        <span className="sr-only">Select {track.title}</span>
        <input type="checkbox" checked={checked} onChange={onToggle} className="h-4 w-4 accent-primary" />
      </label>
      <CoverArt path={track.coverPath} title={track.title} size="md" />
      <div className="min-w-0 flex-1">
        <p className="truncate text-sm font-medium">{track.title}</p>
        <p className="truncate text-xs text-muted-foreground">
          {track.artist}
          {track.durationMs ? ` · ${formatTime(track.durationMs / 1000)}` : ""}
        </p>
        <span
          className={`mt-1 inline-flex rounded-full px-2 py-0.5 text-[10px] uppercase tracking-wide ${
            ready ? "bg-orange-100 text-orange-800 dark:bg-orange-950 dark:text-orange-200" : "bg-secondary text-muted-foreground"
          }`}
        >
          {track.status}
        </span>
        {track.error ? <p className="mt-1 truncate text-xs text-destructive">{track.error}</p> : null}
      </div>
      <div className="flex items-center gap-1 opacity-100 transition-opacity duration-200 lg:opacity-0 lg:group-hover:opacity-100 lg:group-focus-within:opacity-100">
        <Button size="icon" variant={active ? "solid" : "ghost"} aria-label={`Play ${track.title}`} onClick={onPlay}>
          <Play className="h-4 w-4 fill-current" />
        </Button>
        <Button size="icon" variant="ghost" disabled={!ready} aria-label={`Add ${track.title} to queue`} onClick={onQueue}>
          <ListPlus className="h-4 w-4" />
        </Button>
        <Button
          size="icon"
          variant="ghost"
          disabled={!ready}
          aria-label={`Export ${track.title}`}
          onClick={async () => {
            const dest = await open({ directory: true });
            if (typeof dest === "string") {
              await exportStems(track.id, dest);
            }
          }}
        >
          <Download className="h-4 w-4" />
        </Button>
        <Button size="icon" variant="ghost" aria-label={`Delete ${track.title}`} onClick={() => void onDelete()}>
          <Trash2 className="h-4 w-4" />
        </Button>
      </div>
    </li>
  );
}
