#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEST="$ROOT/src-tauri/binaries"
mkdir -p "$DEST"

fetch() {
  local url="$1"
  local out="$2"
  if [[ -f "$out" && -s "$out" ]]; then
    echo "already have $(basename "$out")"
    return
  fi
  echo "downloading $(basename "$out")"
  curl -fsSL -L -o "$out.partial" "$url"
  mv "$out.partial" "$out"
  chmod +x "$out" || true
}

uname_s="$(uname -s)"
uname_m="$(uname -m)"
if [[ "$uname_s" == "Linux" && "$uname_m" == "x86_64" ]]; then
  fetch "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_linux" \
    "$DEST/yt-dlp-x86_64-unknown-linux-gnu"
elif [[ "$uname_s" == "Linux" && "$uname_m" == "aarch64" ]]; then
  fetch "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_linux_aarch64" \
    "$DEST/yt-dlp-aarch64-unknown-linux-gnu"
fi

if [[ "${FETCH_ALL:-0}" == "1" ]]; then
  fetch "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_linux" \
    "$DEST/yt-dlp-x86_64-unknown-linux-gnu"
  fetch "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_linux_aarch64" \
    "$DEST/yt-dlp-aarch64-unknown-linux-gnu"
  fetch "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe" \
    "$DEST/yt-dlp-x86_64-pc-windows-msvc.exe"
fi

echo "sidecars ready in $DEST"
