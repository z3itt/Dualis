export const STORAGE_KEY = "dualis:ui";

export function readUiStorage(): string {
  try {
    return localStorage.getItem(STORAGE_KEY) ?? "{}";
  } catch {
    /* ignore quota / private mode */
  }
  return "{}";
}
