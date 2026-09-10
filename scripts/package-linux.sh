#!/usr/bin/env bash
set -euo pipefail

# Build Dualis .deb / .rpm from the release binary and sidecar.
# AppImage is produced separately (appimagetool on Dualis.AppDir).

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VERSION="${VERSION:-1.0.4}"
RELEASE="${RELEASE:-1}"
ARCH_DEB="${ARCH_DEB:-amd64}"
ARCH_RPM="${ARCH_RPM:-x86_64}"
BIN="$ROOT/src-tauri/target/release/dualis"
DAWN="$ROOT/src-tauri/target/release/libwebgpu_dawn.so"
YTDLP="$ROOT/src-tauri/binaries/yt-dlp-x86_64-unknown-linux-gnu"
ICON512="$ROOT/src-tauri/icons/128x128@2x.png"
ICON128="$ROOT/src-tauri/icons/128x128.png"
ICON64="$ROOT/src-tauri/icons/64x64.png"
ICON32="$ROOT/src-tauri/icons/32x32.png"
LICENSE="$ROOT/LICENSE"
COPYING="$ROOT/COPYING"
OUT_DEB="$ROOT/src-tauri/target/release/bundle/deb"
OUT_RPM="$ROOT/src-tauri/target/release/bundle/rpm"
OUT_APPIMAGE="$ROOT/src-tauri/target/release/bundle/appimage"
STAGE="$ROOT/src-tauri/target/release/bundle/linux-pkg"

# Drop stale installer artifacts from earlier versions.
rm -f "$OUT_DEB"/Dualis_*.deb "$OUT_RPM"/dualis-*.rpm "$OUT_RPM"/Dualis-*.rpm "$OUT_APPIMAGE"/Dualis_*.AppImage
rm -rf "$OUT_DEB"/Dualis_*_amd64 "$OUT_RPM"/Dualis-*-1.x86_64

if [[ ! -x "$BIN" ]]; then
  echo "missing release binary: $BIN" >&2
  exit 1
fi
if [[ ! -f "$DAWN" ]]; then
  echo "missing libwebgpu_dawn.so: $DAWN" >&2
  exit 1
fi
if [[ ! -x "$YTDLP" ]]; then
  echo "missing yt-dlp sidecar: $YTDLP" >&2
  exit 1
fi

rm -rf "$STAGE"
mkdir -p \
  "$STAGE/usr/bin" \
  "$STAGE/usr/lib" \
  "$STAGE/usr/share/applications" \
  "$STAGE/usr/share/doc/dualis" \
  "$STAGE/usr/share/icons/hicolor/32x32/apps" \
  "$STAGE/usr/share/icons/hicolor/64x64/apps" \
  "$STAGE/usr/share/icons/hicolor/128x128/apps" \
  "$STAGE/usr/share/icons/hicolor/256x256/apps" \
  "$OUT_DEB" \
  "$OUT_RPM"

install -m 0755 "$BIN" "$STAGE/usr/bin/dualis"
install -m 0755 "$YTDLP" "$STAGE/usr/bin/yt-dlp"
install -m 0644 "$DAWN" "$STAGE/usr/lib/libwebgpu_dawn.so"
install -m 0644 "$LICENSE" "$STAGE/usr/share/doc/dualis/copyright"
if [[ -f "$COPYING" ]]; then
  install -m 0644 "$COPYING" "$STAGE/usr/share/doc/dualis/COPYING"
fi
[[ -f "$ICON32" ]] && install -m 0644 "$ICON32" "$STAGE/usr/share/icons/hicolor/32x32/apps/dualis.png"
[[ -f "$ICON64" ]] && install -m 0644 "$ICON64" "$STAGE/usr/share/icons/hicolor/64x64/apps/dualis.png"
[[ -f "$ICON128" ]] && install -m 0644 "$ICON128" "$STAGE/usr/share/icons/hicolor/128x128/apps/dualis.png"
[[ -f "$ICON512" ]] && install -m 0644 "$ICON512" "$STAGE/usr/share/icons/hicolor/256x256/apps/dualis.png"

cat > "$STAGE/usr/share/applications/Dualis.desktop" <<'EOF'
[Desktop Entry]
Categories=AudioVideo;Audio;Music;
Comment=On-device vocal and instrumental stem separation
Exec=dualis
StartupWMClass=dualis
Icon=dualis
Name=Dualis
Terminal=false
Type=Application
EOF

INSTALLED_SIZE="$(du -sk "$STAGE" | awk '{print $1}')"

CONTROL="$STAGE/DEBIAN"
mkdir -p "$CONTROL"
cat > "$CONTROL/control" <<EOF
Package: dualis
Version: ${VERSION}
Section: sound
Priority: optional
Architecture: ${ARCH_DEB}
Maintainer: z3itt <info@z3itt.com>
Installed-Size: ${INSTALLED_SIZE}
Depends: libwebkit2gtk-4.1-0 | libwebkit2gtk-4.1-0t64, libgtk-3-0 | libgtk-3-0t64, libglib2.0-0 | libglib2.0-0t64
Homepage: https://github.com/z3itt/Dualis
Description: On-device vocal and instrumental stem separation
 Paste a Spotify or YouTube Music link, download audio locally,
 split vocals from the mix with ONNX, and play both stems in sync.
EOF

DEB_NAME="Dualis_${VERSION}_${ARCH_DEB}.deb"
(
  cd "$STAGE"
  tar --owner=0 --group=0 -czf "$OUT_DEB/data.tar.gz" usr
  tar --owner=0 --group=0 -czf "$OUT_DEB/control.tar.gz" -C DEBIAN control
)
printf '2.0\n' > "$OUT_DEB/debian-binary"
rm -f "$OUT_DEB/$DEB_NAME"
ar r "$OUT_DEB/$DEB_NAME" \
  "$OUT_DEB/debian-binary" \
  "$OUT_DEB/control.tar.gz" \
  "$OUT_DEB/data.tar.gz"
rm -f "$OUT_DEB/debian-binary" "$OUT_DEB/control.tar.gz" "$OUT_DEB/data.tar.gz"
echo "wrote $OUT_DEB/$DEB_NAME"

RPMBUILD="${RPMBUILD:-$HOME/.cache/rpm-build-extract/root/usr/bin/rpmbuild}"
if [[ ! -x "$RPMBUILD" ]] && command -v rpmbuild >/dev/null 2>&1; then
  RPMBUILD="$(command -v rpmbuild)"
fi
if [[ ! -x "$RPMBUILD" ]]; then
  echo "rpmbuild not found; skip RPM. Install rpm-build or extract it under ~/.cache/rpm-build-extract." >&2
else
  TOP="$OUT_RPM/rpmbuild"
  rm -rf "$TOP"
  mkdir -p "$TOP"/{BUILD,RPMS,SOURCES,SPECS,SRPMS}
  cat > "$TOP/SPECS/dualis.spec" <<EOF
Name:           dualis
Version:        ${VERSION}
Release:        ${RELEASE}
Summary:        On-device vocal and instrumental stem separation
License:        GPL-3.0-or-later
URL:            https://github.com/z3itt/Dualis
BuildArch:      ${ARCH_RPM}
AutoReqProv:    no

%global __os_install_post %{nil}
%global __brp_compress %{nil}
%global __brp_strip %{nil}
%global __check_files %{nil}

%description
Paste a Spotify or YouTube Music link, download audio locally,
split vocals from the mix with ONNX, and play both stems in sync.

%install
mkdir -p %{buildroot}
cp -a %{_sourcedir}/payload/. %{buildroot}/

%files
/usr/bin/dualis
/usr/bin/yt-dlp
/usr/lib/libwebgpu_dawn.so
/usr/share/applications/Dualis.desktop
/usr/share/icons/hicolor/32x32/apps/dualis.png
/usr/share/icons/hicolor/64x64/apps/dualis.png
/usr/share/icons/hicolor/128x128/apps/dualis.png
/usr/share/icons/hicolor/256x256/apps/dualis.png
/usr/share/doc/dualis/copyright
/usr/share/doc/dualis/COPYING
EOF
  mkdir -p "$TOP/SOURCES/payload"
  cp -a "$STAGE/usr" "$TOP/SOURCES/payload/"
  "$RPMBUILD" \
    --define "_topdir $TOP" \
    --define "_buildhost localhost" \
    --define "__os_install_post %{nil}" \
    --define "__check_files %{nil}" \
    -bb "$TOP/SPECS/dualis.spec"
  cp -f "$TOP"/RPMS/"$ARCH_RPM"/*.rpm "$OUT_RPM/"
  echo "wrote $OUT_RPM/dualis-${VERSION}-${RELEASE}.${ARCH_RPM}.rpm"
fi

# AppImage: reuse a linuxdeploy AppDir when present, otherwise stage a slim tree.
APPDIR="$OUT_APPIMAGE/Dualis.AppDir"
mkdir -p "$OUT_APPIMAGE"
if [[ ! -d "$APPDIR/usr/bin" ]]; then
  mkdir -p "$APPDIR/usr"
  cp -a "$STAGE/usr/." "$APPDIR/usr/"
  cat > "$APPDIR/AppRun" <<'EOF'
#!/bin/sh
HERE="$(dirname "$(readlink -f "$0")")"
export LD_LIBRARY_PATH="$HERE/usr/lib:${LD_LIBRARY_PATH:-}"
exec "$HERE/usr/bin/dualis" "$@"
EOF
  chmod +x "$APPDIR/AppRun"
  ln -sfn usr/share/applications/Dualis.desktop "$APPDIR/Dualis.desktop"
fi
install -m 0755 "$BIN" "$APPDIR/usr/bin/dualis"
install -m 0755 "$YTDLP" "$APPDIR/usr/bin/yt-dlp"
install -m 0644 "$DAWN" "$APPDIR/usr/lib/libwebgpu_dawn.so"
if [[ -f "$APPDIR/usr/share/icons/hicolor/256x256/apps/dualis.png" ]]; then
  cp -f "$APPDIR/usr/share/icons/hicolor/256x256/apps/dualis.png" "$APPDIR/dualis.png"
elif [[ -f "$ICON512" ]]; then
  cp -f "$ICON512" "$APPDIR/dualis.png"
fi
ln -sfn dualis.png "$APPDIR/.DirIcon"
APPIMAGETOOL="${APPIMAGETOOL:-$HOME/.cache/tauri/linuxdeploy-extracted/squashfs-root/plugins/linuxdeploy-plugin-appimage/usr/bin/appimagetool}"
APPIMAGE_OUT="$OUT_APPIMAGE/Dualis_${VERSION}_amd64.AppImage"
if [[ -x "$APPIMAGETOOL" ]]; then
  export APPIMAGE_EXTRACT_AND_RUN=1
  export ARCH=x86_64
  rm -f "$APPIMAGE_OUT"
  "$APPIMAGETOOL" -n "$APPDIR" "$APPIMAGE_OUT"
  chmod +x "$APPIMAGE_OUT"
  echo "wrote $APPIMAGE_OUT"
else
  echo "appimagetool not found; skip AppImage ($APPIMAGETOOL)" >&2
fi
