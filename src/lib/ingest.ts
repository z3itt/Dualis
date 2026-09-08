export function splitInputs(value: string) {
  return value
    .split(/[\n,]+/)
    .map((item) => item.trim())
    .filter(Boolean);
}

export function looksLikePlaylist(value: string) {
  const lower = value.toLowerCase();
  if (lower.includes("/watch") && lower.includes("v=")) {
    return false;
  }
  return (
    lower.includes("/playlist") ||
    lower.includes("list=") ||
    lower.includes("/browse/vl") ||
    lower.includes("open.spotify.com/playlist") ||
    lower.includes("open.spotify.com/album") ||
    lower.includes("spotify:playlist:") ||
    lower.includes("spotify:album:")
  );
}
