export type StemMode = "original" | "vocals" | "instrumental";

export type LoopMode = "off" | "queue" | "song";

export type TrackStatus =
  | "queued"
  | "downloading"
  | "downloaded"
  | "separating"
  | "ready"
  | "error";

export type LibrarySort = "recent" | "title" | "artist" | "duration" | "status";

export interface Track {
  id: string;
  title: string;
  artist: string;
  sourceUrl?: string | null;
  sourceKind: string;
  sourcePath?: string | null;
  vocalsPath?: string | null;
  instrumentalPath?: string | null;
  coverPath?: string | null;
  durationMs?: number | null;
  sampleRate?: number | null;
  status: TrackStatus | string;
  error?: string | null;
  createdAt: number;
  updatedAt: number;
  peaks?: number[] | null;
  playlistId?: string | null;
  playlistIndex?: number | null;
  ytdlpQuery?: string | null;
}

export interface Playlist {
  id: string;
  title: string;
  artist: string;
  sourceUrl?: string | null;
  sourceKind: string;
  coverPath?: string | null;
  createdAt: number;
  updatedAt: number;
  trackCount: number;
  readyCount: number;
}

export interface LibrarySnapshot {
  tracks: Track[];
  playlists: Playlist[];
}

export interface JobEvent {
  trackId: string;
  stage: string;
  progress: number;
  message: string;
  status: string;
  etaSeconds?: number | null;
}

export interface ModelInfo {
  id: string;
  name: string;
  architecture: string;
  ready: boolean;
  description: string;
}

export interface RuntimeInfo {
  ytdlpPath?: string | null;
  ytdlpSource?: string;
  modelPath?: string | null;
  modelReady: boolean;
  selectedModel?: string;
  executionProvider: string;
  compiledProviders?: string[];
  models?: ModelInfo[];
  cookiesBrowser?: string;
  cookiesFile?: string | null;
  downloadFormat?: string;
  playbackPort?: number;
}

export interface AppErrorLog {
  id: string;
  trackId?: string;
  message: string;
  at: number;
}
