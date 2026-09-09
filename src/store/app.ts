import { create } from "zustand";
import { DualStemEngine } from "@/audio/engine";
import { getLibrary, prioritizeTrack, stemUrl } from "@/lib/api";
import { pruneDismissedJobErrors, tracksInPlaylist } from "@/lib/library";
import { readUiStorage, STORAGE_KEY } from "@/lib/storage";
import type { AppErrorLog, JobEvent, LibrarySort, LoopMode, Playlist, RuntimeInfo, StemMode, Track } from "@/lib/types";

const engine = new DualStemEngine();

interface PersistedUi {
  volume?: number;
  mode?: StemMode;
  sort?: LibrarySort;
  queue?: string[];
  theme?: "light" | "dark";
  shuffle?: boolean;
  loopMode?: LoopMode;
  dismissedJobErrorIds?: string[];
}

function parseLoopMode(value: unknown): LoopMode {
  return value === "queue" || value === "song" ? value : "off";
}

function shuffledIds(ids: string[]) {
  const copy = [...ids];
  for (let i = copy.length - 1; i > 0; i -= 1) {
    const j = Math.floor(Math.random() * (i + 1));
    [copy[i], copy[j]] = [copy[j], copy[i]];
  }
  return copy;
}

function rotateTo(ids: string[], startId: string | null) {
  if (!startId) {
    return ids;
  }
  const index = ids.indexOf(startId);
  if (index <= 0) {
    return ids;
  }
  return [...ids.slice(index), ...ids.slice(0, index)];
}

function playbackList(tracks: Track[], queue: string[], currentId: string | null, lockedContext = "") {
  if (lockedContext.startsWith("playlist:")) {
    const playlistId = lockedContext.slice("playlist:".length);
    const ids = tracksInPlaylist(tracks, playlistId)
      .filter((track) => track.status === "ready")
      .map((track) => track.id);
    if (ids.length > 0) {
      return { ids, context: lockedContext };
    }
  }
  if (lockedContext === "library") {
    return {
      ids: tracks.filter((track) => track.status === "ready").map((track) => track.id),
      context: "library",
    };
  }
  if (lockedContext === "queue") {
    const queued = queue.filter((id) => tracks.find((track) => track.id === id)?.status === "ready");
    if (queued.length > 0) {
      return { ids: queued, context: "queue" };
    }
  }

  const current = tracks.find((track) => track.id === currentId);
  if (current?.playlistId) {
    const ids = tracksInPlaylist(tracks, current.playlistId)
      .filter((track) => track.status === "ready")
      .map((track) => track.id);
    if (ids.length > 0) {
      return { ids, context: `playlist:${current.playlistId}` };
    }
  }
  const queued = queue.filter((id) => tracks.find((track) => track.id === id)?.status === "ready");
  if (queued.length > 0) {
    return { ids: queued, context: "queue" };
  }
  return {
    ids: tracks.filter((track) => track.status === "ready").map((track) => track.id),
    context: "library",
  };
}

function mergeShuffleOrder(order: string[], ids: string[]) {
  const keep = order.filter((id) => ids.includes(id));
  const missing = ids.filter((id) => !keep.includes(id));
  if (missing.length === 0) {
    return keep;
  }
  return [...keep, ...shuffledIds(missing)];
}

function nextInOrder(ids: string[], currentId: string | null, delta: number, wrap: boolean) {
  if (ids.length === 0) {
    return undefined;
  }
  const index = currentId ? ids.indexOf(currentId) : -1;
  if (index < 0) {
    return wrap ? ids[0] : undefined;
  }
  const next = index + delta;
  if (next >= 0 && next < ids.length) {
    return ids[next];
  }
  if (!wrap) {
    return undefined;
  }
  return delta > 0 ? ids[0] : ids[ids.length - 1];
}

let advanceLock = false;

function loadUi(): PersistedUi {
  try {
    return JSON.parse(readUiStorage()) as PersistedUi;
  } catch {
    return {};
  }
}

function saveUi(partial: PersistedUi) {
  try {
    const current = loadUi();
    localStorage.setItem(STORAGE_KEY, JSON.stringify({ ...current, ...partial }));
  } catch {
    /* ignore quota */
  }
}

const initial = loadUi();

interface AppStore {
  tracks: Track[];
  playlists: Playlist[];
  openPlaylistId: string | null;
  jobs: Record<string, JobEvent>;
  currentId: string | null;
  playing: boolean;
  currentTime: number;
  duration: number;
  volume: number;
  mode: StemMode;
  shuffle: boolean;
  loopMode: LoopMode;
  shuffleOrder: string[];
  shufflePlayed: string[];
  shuffleContext: string;
  playHistory: string[];
  query: string;
  sort: LibrarySort;
  selectedIds: string[];
  queue: string[];
  queueOpen: boolean;
  errors: AppErrorLog[];
  dismissedJobErrorIds: string[];
  runtime: RuntimeInfo | null;
  waveform: number[];
  modelBusy: boolean;
  theme: "light" | "dark";
  setQuery: (query: string) => void;
  setSort: (sort: LibrarySort) => void;
  setRuntime: (runtime: RuntimeInfo) => void;
  setModelBusy: (busy: boolean) => void;
  refreshLibrary: () => Promise<void>;
  applyTrack: (track: Track) => void;
  applyJob: (event: JobEvent) => void;
  openPlaylist: (id: string | null) => void;
  removePlaylist: (id: string) => void;
  removeTrack: (id: string) => void;
  removeTracks: (ids: string[]) => void;
  toggleSelected: (id: string) => void;
  clearSelected: () => void;
  selectReady: () => void;
  playTrack: (track: Track, options?: { fromHistory?: boolean; fromSkip?: boolean }) => Promise<void>;
  togglePlay: () => Promise<void>;
  seek: (seconds: number) => void;
  setVolume: (volume: number) => void;
  setMode: (mode: StemMode) => void;
  setShuffle: (shuffle: boolean) => void;
  cycleLoopMode: () => void;
  addToQueue: (id: string) => void;
  removeFromQueue: (id: string) => void;
  clearQueue: () => void;
  setQueueOpen: (open: boolean) => void;
  skip: (delta: number, options?: { fromEnded?: boolean }) => Promise<void>;
  advanceFromEnded: () => Promise<void>;
  dismissError: (id: string) => void;
  dismissJobError: (trackId: string) => void;
  clearDismissedJobError: (trackId: string) => void;
  clearErrors: () => void;
  tick: () => void;
  setTheme: (theme: "light" | "dark") => void;
  initTheme: () => void;
}

export const useAppStore = create<AppStore>((set, get) => ({
  tracks: [],
  playlists: [],
  openPlaylistId: null,
  jobs: {},
  currentId: null,
  playing: false,
  currentTime: 0,
  duration: 0,
  volume: initial.volume ?? 0.9,
  mode: initial.mode ?? "original",
  shuffle: Boolean(initial.shuffle),
  loopMode: parseLoopMode(initial.loopMode),
  shuffleOrder: [],
  shufflePlayed: [],
  shuffleContext: "",
  playHistory: [],
  query: "",
  sort: initial.sort ?? "recent",
  selectedIds: [],
  queue: initial.queue ?? [],
  queueOpen: false,
  errors: [],
  dismissedJobErrorIds: initial.dismissedJobErrorIds ?? [],
  runtime: null,
  waveform: Array.from({ length: 128 }, () => 0.12),
  modelBusy: false,
  theme: initial.theme === "dark" ? "dark" : "light",
  setQuery: (query) => set({ query }),
  setSort: (sort) => {
    saveUi({ sort });
    set({ sort });
  },
  setRuntime: (runtime) => set({ runtime }),
  setModelBusy: (modelBusy) => set({ modelBusy }),
  refreshLibrary: async () => {
    const snapshot = await getLibrary();
    set((state) => {
      const dismissedJobErrorIds = pruneDismissedJobErrors(snapshot.tracks, state.dismissedJobErrorIds);
      if (dismissedJobErrorIds.length !== state.dismissedJobErrorIds.length) {
        saveUi({ dismissedJobErrorIds });
      }
      return { tracks: snapshot.tracks, playlists: snapshot.playlists, dismissedJobErrorIds };
    });
  },
  openPlaylist: (openPlaylistId) => set({ openPlaylistId, selectedIds: [] }),
  removePlaylist: (id) => {
    set((state) => {
      const drop = new Set(state.tracks.filter((track) => track.playlistId === id).map((track) => track.id));
      const queue = state.queue.filter((item) => !drop.has(item));
      saveUi({ queue });
      return {
        playlists: state.playlists.filter((item) => item.id !== id),
        tracks: state.tracks.filter((track) => track.playlistId !== id),
        openPlaylistId: state.openPlaylistId === id ? null : state.openPlaylistId,
        selectedIds: state.selectedIds.filter((item) => !drop.has(item)),
        queue,
        shuffleOrder: state.shuffleOrder.filter((item) => !drop.has(item)),
        shufflePlayed: state.shufflePlayed.filter((item) => !drop.has(item)),
        currentId: state.currentId && drop.has(state.currentId) ? null : state.currentId,
      };
    });
  },
  applyTrack: (track) => {
    set((state) => {
      const index = state.tracks.findIndex((item) => item.id === track.id);
      const tracks =
        index >= 0
          ? state.tracks.map((item) => (item.id === track.id ? { ...item, ...track } : item))
          : [track, ...state.tracks];
      return { tracks };
    });
  },
  applyJob: (event) => {
    set((state) => {
      let dismissedJobErrorIds = state.dismissedJobErrorIds;
      if (event.status === "error") {
        if (dismissedJobErrorIds.includes(event.trackId)) {
          dismissedJobErrorIds = dismissedJobErrorIds.filter((id) => id !== event.trackId);
          saveUi({ dismissedJobErrorIds });
        }
      } else if (dismissedJobErrorIds.includes(event.trackId)) {
        dismissedJobErrorIds = dismissedJobErrorIds.filter((id) => id !== event.trackId);
        saveUi({ dismissedJobErrorIds });
      }
      const errors =
        event.status === "error"
          ? [
              { id: `${event.trackId}-${Date.now()}`, trackId: event.trackId, message: event.message, at: Date.now() },
              ...state.errors,
            ].slice(0, 20)
          : state.errors;
      const tracks = state.tracks.map((track) =>
        track.id === event.trackId
          ? { ...track, status: event.status, error: event.status === "error" ? event.message : track.error }
          : track
      );
      const playlists = state.playlists.map((playlist) => {
        const members = tracks.filter((track) => track.playlistId === playlist.id);
        if (members.length === 0) {
          return playlist;
        }
        return {
          ...playlist,
          trackCount: members.length,
          readyCount: members.filter((track) => track.status === "ready").length,
        };
      });
      return {
        jobs: { ...state.jobs, [event.trackId]: event },
        errors,
        dismissedJobErrorIds,
        tracks,
        playlists,
      };
    });
  },
  removeTrack: (id) => {
    set((state) => {
      const queue = state.queue.filter((item) => item !== id);
      const dismissedJobErrorIds = state.dismissedJobErrorIds.filter((item) => item !== id);
      saveUi({ queue, dismissedJobErrorIds });
      return {
        tracks: state.tracks.filter((track) => track.id !== id),
        selectedIds: state.selectedIds.filter((item) => item !== id),
        queue,
        dismissedJobErrorIds,
        shuffleOrder: state.shuffleOrder.filter((item) => item !== id),
        shufflePlayed: state.shufflePlayed.filter((item) => item !== id),
        currentId: state.currentId === id ? null : state.currentId,
      };
    });
  },
  removeTracks: (ids) => {
    const drop = new Set(ids);
    set((state) => {
      const queue = state.queue.filter((item) => !drop.has(item));
      const dismissedJobErrorIds = state.dismissedJobErrorIds.filter((item) => !drop.has(item));
      saveUi({ queue, dismissedJobErrorIds });
      return {
        tracks: state.tracks.filter((track) => !drop.has(track.id)),
        selectedIds: [],
        queue,
        dismissedJobErrorIds,
        shuffleOrder: state.shuffleOrder.filter((item) => !drop.has(item)),
        shufflePlayed: state.shufflePlayed.filter((item) => !drop.has(item)),
        currentId: state.currentId && drop.has(state.currentId) ? null : state.currentId,
      };
    });
  },
  toggleSelected: (id) => {
    set((state) => ({
      selectedIds: state.selectedIds.includes(id)
        ? state.selectedIds.filter((item) => item !== id)
        : [...state.selectedIds, id],
    }));
  },
  clearSelected: () => set({ selectedIds: [] }),
  selectReady: () => {
    set((state) => ({
      selectedIds: state.tracks.filter((track) => track.status === "ready").map((track) => track.id),
    }));
  },
  playTrack: async (track, options) => {
    if (track.status !== "ready" || !track.vocalsPath || !track.instrumentalPath) {
      try {
        await prioritizeTrack(track.id);
        set((state) => ({
          errors: [
            {
              id: `prep-${track.id}-${Date.now()}`,
              trackId: track.id,
              message: `${track.title} is next in the queue. Playback starts when stems are ready.`,
              at: Date.now(),
            },
            ...state.errors,
          ].slice(0, 20),
        }));
      } catch (err) {
        const message = err instanceof Error ? err.message : "Could not queue this track";
        set((state) => ({
          errors: [
            { id: `prep-${track.id}-${Date.now()}`, trackId: track.id, message, at: Date.now() },
            ...state.errors,
          ].slice(0, 20),
        }));
      }
      return;
    }
    const port = get().runtime?.playbackPort;
    const vocals = stemUrl(port, track.vocalsPath);
    const instrumental = stemUrl(port, track.instrumentalPath);
    if (!vocals || !instrumental) {
      set((state) => ({
        errors: [
          {
            id: `play-${track.id}-${Date.now()}`,
            trackId: track.id,
            message: "Playback server is not ready yet. Restart the app and try again.",
            at: Date.now(),
          },
          ...state.errors,
        ].slice(0, 20),
      }));
      return;
    }
    set((state) => {
      const queue = state.queue.includes(track.id) ? state.queue : [...state.queue, track.id];
      saveUi({ queue });
      const playHistory =
        !options?.fromHistory && state.currentId && state.currentId !== track.id
          ? [...state.playHistory, state.currentId].slice(-80)
          : state.playHistory;
      let shuffleOrder = state.shuffleOrder;
      let shufflePlayed = state.shufflePlayed;
      let shuffleContext = state.shuffleContext;
      if (state.shuffle && !options?.fromSkip && !options?.fromHistory) {
        const list = playbackList(state.tracks, queue, track.id);
        shuffleOrder = rotateTo(shuffledIds(list.ids), track.id);
        shufflePlayed = [];
        shuffleContext = list.context;
      }
      return {
        currentId: track.id,
        playHistory,
        shuffleOrder,
        shufflePlayed,
        shuffleContext,
        playing: false,
        currentTime: 0,
        duration: track.durationMs ? track.durationMs / 1000 : 0,
        waveform: track.peaks && track.peaks.length > 8 ? track.peaks : state.waveform,
        queue,
      };
    });
    try {
      await engine.load(vocals, instrumental);
      engine.setMode(get().mode);
      engine.setVolume(get().volume);
      engine.setLoop(get().loopMode === "song");
      await engine.play();
      set({
        playing: true,
        duration: engine.duration || get().duration,
        waveform: track.peaks && track.peaks.length > 8 ? track.peaks : engine.waveform(),
      });
    } catch (err) {
      const message = err instanceof Error ? err.message : "Could not play stems";
      set((state) => ({
        playing: false,
        errors: [
          { id: `play-${track.id}-${Date.now()}`, trackId: track.id, message, at: Date.now() },
          ...state.errors,
        ].slice(0, 20),
      }));
    }
  },
  togglePlay: async () => {
    if (!get().currentId) {
      const ready = get().tracks.find((track) => track.status === "ready");
      if (ready) {
        await get().playTrack(ready);
      }
      return;
    }
    if (engine.playing) {
      engine.pause();
      set({ playing: false, currentTime: engine.currentTime() });
      return;
    }
    await engine.play();
    set({ playing: true });
  },
  seek: (seconds) => {
    engine.seek(seconds);
    set({ currentTime: engine.currentTime(), playing: engine.playing });
  },
  setVolume: (volume) => {
    engine.setVolume(volume);
    saveUi({ volume });
    set({ volume });
  },
  setMode: (mode) => {
    engine.setMode(mode);
    saveUi({ mode });
    set({ mode });
  },
  setShuffle: (shuffle) => {
    saveUi({ shuffle });
    const { tracks, queue, currentId } = get();
    const list = playbackList(tracks, queue, currentId);
    set({
      shuffle,
      shuffleOrder: shuffle ? rotateTo(shuffledIds(list.ids), currentId) : [],
      shufflePlayed: [],
      shuffleContext: shuffle ? list.context : "",
    });
  },
  cycleLoopMode: () => {
    const next: LoopMode = get().loopMode === "off" ? "queue" : get().loopMode === "queue" ? "song" : "off";
    saveUi({ loopMode: next });
    engine.setLoop(next === "song");
    set({ loopMode: next });
  },
  addToQueue: (id) => {
    set((state) => {
      if (state.queue.includes(id)) {
        return state;
      }
      const queue = [...state.queue, id];
      saveUi({ queue });
      return { queue };
    });
  },
  removeFromQueue: (id) => {
    set((state) => {
      const queue = state.queue.filter((item) => item !== id);
      saveUi({ queue });
      return { queue };
    });
  },
  clearQueue: () => {
    saveUi({ queue: [] });
    set({ queue: [] });
  },
  setQueueOpen: (queueOpen) => set({ queueOpen }),
  skip: async (delta, options) => {
    const fromEnded = Boolean(options?.fromEnded);
    const { queue, tracks, currentId, playTrack, shuffle, loopMode, playHistory } = get();
    const list = playbackList(tracks, queue, currentId, shuffle ? get().shuffleContext : "");
    const readyIds = list.ids;

    if (delta < 0 && playHistory.length > 0) {
      const prevId = playHistory[playHistory.length - 1];
      const prev = tracks.find((track) => track.id === prevId && track.status === "ready");
      set({ playHistory: playHistory.slice(0, -1) });
      if (prev) {
        await playTrack(prev, { fromHistory: true });
        return;
      }
    }

    let order = readyIds;
    let played = get().shufflePlayed.filter((id) => readyIds.includes(id));

    if (shuffle) {
      const existing = get().shuffleOrder;
      if (get().shuffleContext !== list.context || existing.length === 0) {
        order = rotateTo(shuffledIds(readyIds), currentId);
        played = [];
      } else {
        order = mergeShuffleOrder(existing, readyIds);
      }
      set({ shuffleOrder: order, shufflePlayed: played, shuffleContext: list.context });
    }

    const wrap = !fromEnded || loopMode === "queue";
    let nextId: string | undefined;

    if (shuffle && fromEnded) {
      if (currentId && !played.includes(currentId)) {
        played = [...played, currentId];
      }
      nextId = order.find((id) => !played.includes(id));
      if (!nextId && loopMode === "queue" && readyIds.length > 0) {
        order = shuffledIds(readyIds);
        const start = order[0] === currentId && order.length > 1 ? (order[1] ?? order[0]) : order[0];
        order = rotateTo(order, start);
        played = [];
        nextId = order[0];
        set({ shuffleOrder: order, shufflePlayed: played });
      } else {
        set({ shufflePlayed: played });
      }
    } else {
      nextId = nextInOrder(order, currentId, delta, wrap);
    }

    if (!nextId) {
      engine.pause();
      set({ playing: false, currentTime: engine.currentTime() });
      return;
    }

    if (nextId === currentId) {
      engine.seek(0);
      await engine.play();
      set({ playing: true, currentTime: 0 });
      return;
    }

    const next = tracks.find((track) => track.id === nextId);
    if (next) {
      await playTrack(next, { fromSkip: true });
    }
  },
  advanceFromEnded: async () => {
    if (advanceLock) {
      return;
    }
    advanceLock = true;
    try {
      if (get().loopMode === "song") {
        engine.seek(0);
        await engine.play();
        set({ playing: true, currentTime: 0 });
        return;
      }
      await get().skip(1, { fromEnded: true });
    } finally {
      advanceLock = false;
    }
  },
  dismissError: (id) => set((state) => ({ errors: state.errors.filter((item) => item.id !== id) })),
  dismissJobError: (trackId) => {
    set((state) => {
      if (state.dismissedJobErrorIds.includes(trackId)) {
        return state;
      }
      const dismissedJobErrorIds = [...state.dismissedJobErrorIds, trackId];
      saveUi({ dismissedJobErrorIds });
      return { dismissedJobErrorIds };
    });
  },
  clearDismissedJobError: (trackId) => {
    set((state) => {
      if (!state.dismissedJobErrorIds.includes(trackId)) {
        return state;
      }
      const dismissedJobErrorIds = state.dismissedJobErrorIds.filter((id) => id !== trackId);
      saveUi({ dismissedJobErrorIds });
      return { dismissedJobErrorIds };
    });
  },
  clearErrors: () => set({ errors: [] }),
  tick: () => {
    engine.pollEnded();
    const shouldAdvance = engine.consumeEnded();
    set({
      playing: engine.playing,
      currentTime: engine.currentTime(),
      duration: engine.duration || get().duration,
    });
    if (shouldAdvance) {
      void get().advanceFromEnded();
    }
  },
  setTheme: (theme) => {
    saveUi({ theme });
    set({ theme });
  },
  initTheme: () => {
    const theme = loadUi().theme === "dark" ? "dark" : "light";
    document.documentElement.classList.toggle("dark", theme === "dark");
    set({ theme });
  },
}));

engine.setLoop(useAppStore.getState().loopMode === "song");
engine.onEnded(() => {
  if (engine.consumeEnded()) {
    void useAppStore.getState().advanceFromEnded();
  }
});
