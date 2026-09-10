import { FormEvent, useEffect, useRef, useState } from "react";
import pkg from "../../package.json";
import { ArrowUpRight, Clock, FolderOpen, Link2, Loader2, Moon, MoreVertical, Sun } from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";
import { BrandMark } from "@/components/BrandMark";
import { Button } from "@/components/ui/button";
import { ingest, ingestLocal, setCookiesBrowser, setCookiesFile, setDownloadFormat, setModel } from "@/lib/api";
import { looksLikePlaylist, splitInputs } from "@/lib/ingest";
import { useAppStore } from "@/store/app";

const BROWSERS = [
  { id: "firefox", label: "Firefox" },
  { id: "chrome", label: "Chrome" },
  { id: "chromium", label: "Chromium" },
  { id: "brave", label: "Brave" },
  { id: "edge", label: "Edge" },
  { id: "none", label: "Off" },
];

const DOWNLOAD_FORMATS = [
  { id: "auto", label: "Auto (recommended)" },
  { id: "best", label: "Best audio" },
  { id: "any", label: "Any available" },
  { id: "fast", label: "Fast / low bandwidth" },
];

export function TopBar({ now }: { now: string }) {
  const runtime = useAppStore((s) => s.runtime);
  const setRuntime = useAppStore((s) => s.setRuntime);
  const theme = useAppStore((s) => s.theme);
  const setTheme = useAppStore((s) => s.setTheme);
  const modelBusy = useAppStore((s) => s.modelBusy);
  const setModelBusy = useAppStore((s) => s.setModelBusy);
  const applyTrack = useAppStore((s) => s.applyTrack);
  const refreshLibrary = useAppStore((s) => s.refreshLibrary);

  const [value, setValue] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [menuOpen, setMenuOpen] = useState(false);
  const [settingsBusy, setSettingsBusy] = useState(false);
  const [modelError, setModelError] = useState<string | null>(null);
  const rootRef = useRef<HTMLDivElement>(null);

  const models = runtime?.models ?? [];
  const playlistHint = splitInputs(value).some(looksLikePlaylist);

  useEffect(() => {
    if (!menuOpen) {
      return;
    }
    function onPointerDown(event: MouseEvent) {
      if (rootRef.current && !rootRef.current.contains(event.target as Node)) {
        setMenuOpen(false);
      }
    }
    function onKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        setMenuOpen(false);
      }
    }
    document.addEventListener("mousedown", onPointerDown);
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("mousedown", onPointerDown);
      document.removeEventListener("keydown", onKeyDown);
    };
  }, [menuOpen]);

  async function onSubmit(event: FormEvent) {
    event.preventDefault();
    const inputs = splitInputs(value);
    if (inputs.length === 0) {
      setError("Paste a Spotify, YouTube Music, or YouTube link.");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      await ingest(inputs);
      await refreshLibrary();
      setValue("");
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  }

  async function onLocal() {
    const selected = await open({
      multiple: true,
      filters: [{ name: "Audio", extensions: ["wav", "flac", "mp3", "m4a", "ogg", "aac"] }],
    });
    const files = Array.isArray(selected) ? selected : selected ? [selected] : [];
    if (files.length === 0) {
      return;
    }
    setBusy(true);
    setError(null);
    try {
      for (const file of files) {
        applyTrack(await ingestLocal(file));
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  }

  async function updateSetting(task: () => Promise<unknown>) {
    setSettingsBusy(true);
    try {
      await task();
    } finally {
      setSettingsBusy(false);
    }
  }

  return (
    <header className="mx-auto w-full p-2 sm:p-3">
      <div className="surface rounded-2xl px-2 py-2 sm:px-3 sm:py-2.5">
        <div className="flex flex-col gap-2 lg:flex-row lg:items-center">
          <div className="flex shrink-0 items-center gap-2.5 lg:w-auto">
            <BrandMark theme={theme} />
            <div className="hidden min-w-0 sm:block" aria-hidden="true">
              <p className="text-sm font-semibold leading-none tracking-[0.16em]">DUΛLIS</p>
              <p className="mt-0.5 text-[11px] text-muted-foreground">Local vocal separation</p>
            </div>
          </div>

          <form onSubmit={onSubmit} className="flex min-w-0 flex-1 flex-col gap-2 sm:flex-row sm:items-center">
            <label className="sr-only" htmlFor="ingest">
              Paste Spotify or YouTube Music links
            </label>
            <div className="relative min-w-0 flex-1">
              <Link2 className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
              <input
                id="ingest"
                value={value}
                onChange={(event) => setValue(event.target.value)}
                placeholder="Paste Spotify, YouTube Music, or playlist links"
                className="field-input h-10 w-full rounded-full border border-border bg-input py-2 pl-10 pr-4 text-sm text-foreground placeholder:text-muted-foreground"
              />
            </div>
            <div className="flex shrink-0 items-center gap-2">
              <Button type="submit" disabled={busy} className="group pr-1.5">
                <span>{busy ? "Working…" : "Separate"}</span>
                <span className="flex h-7 w-7 items-center justify-center rounded-full bg-white/90 text-primary transition-transform duration-300 ease-out group-hover:rotate-45 motion-reduce:transform-none dark:bg-black/25 dark:text-white">
                  {busy ? <Loader2 className="h-4 w-4 animate-spin" /> : <ArrowUpRight className="h-4 w-4" />}
                </span>
              </Button>
              <Button type="button" variant="outline" onClick={onLocal} disabled={busy} aria-label="Open local audio files">
                <FolderOpen className="h-4 w-4" />
                <span className="hidden sm:inline">Files</span>
              </Button>
            </div>
          </form>

          <div className="flex shrink-0 items-center justify-end gap-2 lg:pl-1">
            <div className="hidden items-center gap-1.5 text-xs text-muted-foreground xl:flex">
              <Clock className="h-3.5 w-3.5" />
              <span>{now}</span>
            </div>
            <div ref={rootRef} className="relative">
              <button
                type="button"
                aria-label="Settings"
                aria-expanded={menuOpen}
                aria-haspopup="menu"
                onClick={() => setMenuOpen((open) => !open)}
                className="flex h-9 w-9 items-center justify-center rounded-full border border-border bg-input text-foreground transition-colors hover:bg-muted"
              >
                <MoreVertical className="h-4 w-4" />
              </button>
              {menuOpen ? (
                <div role="menu" className="surface-raised absolute right-0 top-[calc(100%+8px)] z-50 w-72 rounded-2xl p-3">
                  <p className="mb-3 px-1 text-xs font-medium uppercase tracking-wide text-muted-foreground">Settings</p>

                  <label className="mb-3 block px-1">
                    <span className="mb-1.5 block text-xs text-muted-foreground">Separation model</span>
                    <select
                      value={runtime?.selectedModel ?? "kim-vocal-2"}
                      disabled={settingsBusy || modelBusy || models.length === 0}
                      onChange={(event) =>
                        void updateSetting(async () => {
                          setModelError(null);
                          setModelBusy(true);
                          try {
                            setRuntime(await setModel(event.target.value));
                          } catch (err) {
                            setModelError(err instanceof Error ? err.message : String(err));
                          } finally {
                            setModelBusy(false);
                          }
                        })
                      }
                      className="field-input w-full rounded-xl border border-border bg-input px-3 py-2 text-sm text-foreground"
                    >
                      {models.length === 0 ? <option value="kim-vocal-2">Kim Vocal 2</option> : null}
                      {models.map((model) => (
                        <option key={model.id} value={model.id}>
                          {model.name}
                          {model.ready ? "" : " · download"}
                        </option>
                      ))}
                    </select>
                    <span className="mt-1 block text-[11px] leading-snug text-muted-foreground">
                      AI model used to split vocals from instrumentals.
                    </span>
                    {modelError ? <span className="mt-1 block text-[11px] text-destructive">{modelError}</span> : null}
                  </label>

                  <label className="mb-3 block px-1">
                    <span className="mb-1.5 block text-xs text-muted-foreground">YouTube login</span>
                    <select
                      value={runtime?.cookiesBrowser ?? "firefox"}
                      disabled={settingsBusy || Boolean(runtime?.cookiesFile)}
                      onChange={(event) =>
                        void updateSetting(async () => {
                          setRuntime(await setCookiesBrowser(event.target.value));
                        })
                      }
                      className="field-input w-full rounded-xl border border-border bg-input px-3 py-2 text-sm text-foreground"
                    >
                      {BROWSERS.map((browser) => (
                        <option key={browser.id} value={browser.id}>
                          {browser.label}
                        </option>
                      ))}
                    </select>
                  </label>

                  <div className="mb-3 px-1">
                    <span className="mb-1.5 block text-xs text-muted-foreground">Or cookies.txt</span>
                    <div className="flex gap-2">
                      <Button
                        type="button"
                        size="sm"
                        variant="outline"
                        disabled={settingsBusy}
                        className="flex-1"
                        onClick={() =>
                          void updateSetting(async () => {
                            const picked = await open({
                              filters: [{ name: "Cookies", extensions: ["txt"] }],
                              multiple: false,
                            });
                            if (typeof picked === "string") {
                              setRuntime(await setCookiesFile(picked));
                            }
                          })
                        }
                      >
                        Import file
                      </Button>
                      {runtime?.cookiesFile ? (
                        <Button
                          type="button"
                          size="sm"
                          variant="ghost"
                          disabled={settingsBusy}
                          onClick={() =>
                            void updateSetting(async () => {
                              setRuntime(await setCookiesFile(""));
                            })
                          }
                        >
                          Clear
                        </Button>
                      ) : null}
                    </div>
                    {runtime?.cookiesFile ? (
                      <p className="mt-1 truncate text-[11px] text-muted-foreground" title={runtime.cookiesFile}>
                        Using {runtime.cookiesFile.split("/").pop()}
                      </p>
                    ) : (
                      <p className="mt-1 text-[11px] leading-snug text-muted-foreground">
                        Netscape cookies.txt.
                      </p>
                    )}
                  </div>

                  <label className="mb-3 block px-1">
                    <span className="mb-1.5 block text-xs text-muted-foreground">Download quality</span>
                    <select
                      value={runtime?.downloadFormat ?? "auto"}
                      disabled={settingsBusy}
                      onChange={(event) =>
                        void updateSetting(async () => {
                          setRuntime(await setDownloadFormat(event.target.value));
                        })
                      }
                      className="field-input w-full rounded-xl border border-border bg-input px-3 py-2 text-sm text-foreground"
                    >
                      {DOWNLOAD_FORMATS.map((item) => (
                        <option key={item.id} value={item.id}>
                          {item.label}
                        </option>
                      ))}
                    </select>
                  </label>

                  <div className="flex items-center justify-between rounded-xl bg-muted px-3 py-2.5">
                    <div>
                      <p className="text-sm font-medium text-foreground">Appearance</p>
                      <p className="text-xs text-muted-foreground">{theme === "dark" ? "Dark mode" : "Light mode"}</p>
                    </div>
                    <div className="flex rounded-full bg-secondary p-1 ring-1 ring-border">
                      <button
                        type="button"
                        aria-label="Light mode"
                        aria-pressed={theme === "light"}
                        onClick={() => setTheme("light")}
                        className={`flex h-7 w-7 items-center justify-center rounded-full transition-colors duration-200 ${theme === "light" ? "bg-card text-primary shadow-sm" : "text-muted-foreground hover:text-foreground"}`}
                      >
                        <Sun className="h-4 w-4" />
                      </button>
                      <button
                        type="button"
                        aria-label="Dark mode"
                        aria-pressed={theme === "dark"}
                        onClick={() => setTheme("dark")}
                        className={`flex h-7 w-7 items-center justify-center rounded-full transition-colors duration-200 ${theme === "dark" ? "bg-card text-primary shadow-sm" : "text-muted-foreground hover:text-foreground"}`}
                      >
                        <Moon className="h-4 w-4" />
                      </button>
                    </div>
                  </div>

                  {settingsBusy || modelBusy ? (
                    <p className="mt-2 flex items-center gap-2 px-1 text-xs text-muted-foreground">
                      <Loader2 className="h-3.5 w-3.5 animate-spin" />
                      Saving…
                    </p>
                  ) : null}

                  <p className="mt-3 border-t border-border pt-2 text-center text-[10px] text-muted-foreground">
                    developed by z3itt • v{pkg.version}
                  </p>
                </div>
              ) : null}
            </div>
          </div>
        </div>

        {playlistHint ? (
          <p className="mt-2 px-1 text-xs text-muted-foreground">Playlist detected. Tracks will expand into the job queue.</p>
        ) : null}
        {error ? <p className="mt-2 px-1 text-sm text-destructive">{error}</p> : null}
      </div>
    </header>
  );
}
