import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { ErrorToasts } from "@/components/ErrorToasts";
import { JobQueue } from "@/components/JobQueue";
import { PlayerBar } from "@/components/PlayerBar";
import { TopBar } from "@/components/TopBar";
import { TrackLibrary } from "@/components/TrackLibrary";
import { ensureRuntime } from "@/lib/api";
import { applyUiScale } from "@/lib/ui-scale";
import type { JobEvent } from "@/lib/types";
import { useAppStore } from "@/store/app";

export default function App() {
  const refreshLibrary = useAppStore((s) => s.refreshLibrary);
  const applyJob = useAppStore((s) => s.applyJob);
  const setRuntime = useAppStore((s) => s.setRuntime);
  const tick = useAppStore((s) => s.tick);
  const runtime = useAppStore((s) => s.runtime);
  const theme = useAppStore((s) => s.theme);
  const initTheme = useAppStore((s) => s.initTheme);
  const [now, setNow] = useState(() => formatClock());

  useEffect(() => {
    initTheme();
  }, [initTheme]);

  useEffect(() => {
    applyUiScale();
    window.addEventListener("resize", applyUiScale);
    return () => window.removeEventListener("resize", applyUiScale);
  }, []);

  useEffect(() => {
    document.documentElement.classList.toggle("dark", theme === "dark");
    const icon = document.querySelector<HTMLLinkElement>('link[rel="icon"]');
    if (icon) {
      icon.href = theme === "dark" ? "/brand/dualis-dark.jpg" : "/brand/dualis-light.jpg";
    }
  }, [theme]);

  useEffect(() => {
    void refreshLibrary();
    void ensureRuntime()
      .then(setRuntime)
      .catch(() => undefined);
    const timer = window.setInterval(tick, 200);
    const clock = window.setInterval(() => setNow(formatClock()), 1000);
    const unlistenPromise = listen<JobEvent>("job-progress", (event) => {
      applyJob(event.payload);
      if (event.payload.status === "ready" || event.payload.status === "error") {
        void refreshLibrary();
      }
    });
    return () => {
      window.clearInterval(timer);
      window.clearInterval(clock);
      void unlistenPromise.then((fn) => fn());
    };
  }, [applyJob, refreshLibrary, setRuntime, tick]);

  const providers = runtime?.compiledProviders?.join(" / ") ?? "WebGPU / CPU";

  return (
    <div className="flex h-full min-h-0 flex-col overflow-hidden bg-background text-foreground">
      <ErrorToasts />
      <div className="shrink-0">
        <TopBar now={now} />
      </div>

      <main className="mx-auto grid min-h-0 w-full flex-1 grid-cols-1 grid-rows-[minmax(0,auto)_minmax(0,1fr)] gap-4 overflow-hidden px-2 pb-3 sm:px-3 lg:grid-cols-[minmax(15rem,19rem)_minmax(0,1fr)] lg:grid-rows-1 lg:items-stretch">
        <JobQueue />
        <TrackLibrary />
      </main>

      <div className="mx-auto w-full shrink-0 px-2 pb-3 sm:px-3">
        <PlayerBar />
        <p className="mt-2 hidden text-center text-[11px] text-muted-foreground md:block">
          {providers} · 32-bit float WAV · runs fully on your machine
        </p>
      </div>
    </div>
  );
}

function formatClock() {
  return new Intl.DateTimeFormat(undefined, { hour: "2-digit", minute: "2-digit" }).format(new Date());
}
