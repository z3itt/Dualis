export const STORAGE_KEY = "dualis:ui";
const LEGACY_STORAGE_KEY = "stems:ui";

export function readUiStorage(): string {
  try {
    const current = localStorage.getItem(STORAGE_KEY);
    if (current != null) {
      return current;
    }
    const legacy = localStorage.getItem(LEGACY_STORAGE_KEY);
    if (legacy != null) {
      localStorage.setItem(STORAGE_KEY, legacy);
      localStorage.removeItem(LEGACY_STORAGE_KEY);
      return legacy;
    }
  } catch {
    /* ignore quota / private mode */
  }
  return "{}";
}
