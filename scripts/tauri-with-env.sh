#!/usr/bin/env bash
set -euo pipefail

# Fedora/tmpfs: /tmp is often small. Rust C compiler temps and linkers fail with
# "Disk quota exceeded" when TMPDIR points at a full tmpfs. Use home instead.
DUALIS_CACHE="${DUALIS_CACHE:-$HOME/.cache/com.z3itt.dualis-build}"
mkdir -p "$DUALIS_CACHE/tmp"
export TMPDIR="$DUALIS_CACHE/tmp"
export TEMP="$DUALIS_CACHE/tmp"
export TMP="$DUALIS_CACHE/tmp"

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# Keep artifacts in the repo. Cursor/CI often injects a sandbox CARGO_TARGET_DIR.
export CARGO_TARGET_DIR="$ROOT/src-tauri/target"
# AppImages need FUSE; extract-and-run works on Fedora without it.
export APPIMAGE_EXTRACT_AND_RUN=1
export NO_STRIP="${NO_STRIP:-1}"
LINUXDEPLOY_BIN="$HOME/.cache/tauri/linuxdeploy-extracted/squashfs-root/usr/bin/linuxdeploy"
if [[ -x "$LINUXDEPLOY_BIN" ]]; then
  export LINUXDEPLOY="$LINUXDEPLOY_BIN"
  export PATH="$(dirname "$LINUXDEPLOY_BIN"):$HOME/.cache/tauri:${PATH:-}"
fi
# ONNX WebGPU ships libwebgpu_dawn.so next to the binary. linuxdeploy/ldd
# must see it or AppImage/DEB/RPM bundling fails.
export LD_LIBRARY_PATH="$CARGO_TARGET_DIR/release:$CARGO_TARGET_DIR/debug:${LD_LIBRARY_PATH:-}"

# User-local GTK/WebKit devel prefix (no root required on Fedora).
SYSROOT="${TAURI_SYSROOT:-$HOME/.local/tauri-sysroot}"
if [[ -d "$SYSROOT/usr/lib64/pkgconfig" ]]; then
  export PKG_CONFIG_SYSROOT_DIR="$SYSROOT"
  export PKG_CONFIG_PATH="$SYSROOT/usr/lib64/pkgconfig:$SYSROOT/usr/share/pkgconfig:${PKG_CONFIG_PATH:-}"
  export LIBRARY_PATH="$SYSROOT/usr/lib64:/usr/lib64:${LIBRARY_PATH:-}"
  export LD_LIBRARY_PATH="/usr/lib64:${LD_LIBRARY_PATH:-}"
  export RUSTFLAGS="${RUSTFLAGS:-} -C link-arg=-L/usr/lib64 -C link-arg=-L$SYSROOT/usr/lib64"
fi
# Packaged Dualis must find libwebgpu_dawn.so beside the binary or in ../lib.
export RUSTFLAGS="${RUSTFLAGS:-} -C link-arg=-Wl,-rpath,\$ORIGIN -C link-arg=-Wl,-rpath,\$ORIGIN/../lib"

export PATH="$HOME/.cargo/bin:${PATH:-}"
if [[ -x "$HOME/.local/share/fnm/fnm" ]]; then
  eval "$("$HOME/.local/share/fnm/fnm" env --shell bash)"
fi

exec npx tauri "$@"
