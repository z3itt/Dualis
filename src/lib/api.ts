import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import type { JobEvent, LibrarySnapshot, ModelInfo, RuntimeInfo, Track } from "./types";

export async function ingest(inputs: string[]) {
  return invoke<LibrarySnapshot>("ingest", { inputs });
}

export async function ingestLocal(path: string) {
  return invoke<Track>("ingest_local", { path });
}

export async function getLibrary() {
  return invoke<LibrarySnapshot>("get_library");
}

export async function deleteTrack(trackId: string) {
  return invoke<void>("delete_track", { trackId });
}

export async function deleteTracks(trackIds: string[]) {
  return invoke<void>("delete_tracks", { trackIds });
}

export async function separateStems(trackId: string) {
  return invoke("separate_stems", { trackId });
}

export async function retryTrack(trackId: string) {
  return invoke<void>("retry_track", { trackId });
}

export async function prioritizeTrack(trackId: string) {
  return invoke<void>("prioritize_track", { trackId });
}

export async function deletePlaylist(playlistId: string) {
  return invoke<void>("delete_playlist", { playlistId });
}

export async function runtimeInfo() {
  return invoke<RuntimeInfo>("runtime_info");
}

export async function ensureRuntime() {
  return invoke<RuntimeInfo>("ensure_runtime");
}

export async function setModel(modelId: string) {
  return invoke<RuntimeInfo>("set_model", { modelId });
}

export async function setCookiesBrowser(browser: string) {
  return invoke<RuntimeInfo>("set_cookies_browser", { browser });
}

export async function setCookiesFile(path: string) {
  return invoke<RuntimeInfo>("set_cookies_file", { path });
}

export async function setDownloadFormat(format: string) {
  return invoke<RuntimeInfo>("set_download_format", { format });
}

export async function listModels() {
  return invoke<ModelInfo[]>("list_models");
}

export async function exportStems(trackId: string, destDir: string) {
  return invoke<void>("export_stems", { trackId, destDir });
}

export async function exportStemsBatch(trackIds: string[], destDir: string) {
  return invoke<number>("export_stems_batch", { trackIds, destDir });
}

export function stemUrl(port: number | undefined, path?: string | null) {
  if (!port || !path) {
    return "";
  }
  const hex = Array.from(new TextEncoder().encode(path), (byte) => byte.toString(16).padStart(2, "0")).join("");
  return `http://127.0.0.1:${port}/stem/${hex}`;
}

export function fileSrc(path?: string | null) {
  if (!path) {
    return "";
  }
  try {
    return convertFileSrc(path);
  } catch {
    return path;
  }
}

export type { JobEvent };
