# Contributing to Dualis

Dualis is GPL-3.0-or-later. By opening a pull request you agree to license
your contribution under the same terms. See [LICENSE](LICENSE) and
[COPYING](COPYING).

## Setup

```bash
git clone https://github.com/z3itt/Dualis.git
cd Dualis
npm install
npm run sidecar
npm run desktop
```

Linux builds wrap `tauri dev` via `scripts/tauri-with-env.sh` so Fedora `/tmp`
and GTK `pkg-config` resolve.

## Tests

```bash
npm test
cd src-tauri && cargo test --lib
```

## Pull requests

- Keep the change focused. Do not mix refactors with bug fixes.
- Do not commit `cookies.txt`, `.env`, ONNX models, or yt-dlp sidecars.
- Playback on Linux uses the local WAV HTTP server. Do not switch it back to
  `asset://` or Web Audio without a Linux smoke test.
- Spotify ingest must search YouTube as `title artist`, not the Spotify URL.

## Contact

z3itt · info@z3itt.com · https://z3itt.com
