import { useState } from "react";
import { Loader2, RotateCcw, X } from "lucide-react";
import { Progress } from "@/components/ui/progress";
import { Button } from "@/components/ui/button";
import { StageChips } from "@/components/StageChips";
import { retryTrack } from "@/lib/api";
import { parseEtaSeconds } from "@/lib/library";
import { formatEta } from "@/lib/utils";
import { useAppStore } from "@/store/app";

export function JobQueue() {
  const tracks = useAppStore((s) => s.tracks);
  const jobs = useAppStore((s) => s.jobs);
  const applyTrack = useAppStore((s) => s.applyTrack);
  const [hiddenErrors, setHiddenErrors] = useState<string[]>([]);
  const live = tracks.filter((track) => ["downloading", "downloaded", "separating"].includes(String(track.status)));
  const waiting = tracks.filter((track) => track.status === "queued");
  const failedAll = tracks.filter(
    (track) => track.status === "error" && !hiddenErrors.includes(track.id)
  );
  const failed = failedAll.slice(0, 8);

  return (
    <section className="panel surface flex max-h-[34vh] min-h-0 w-full flex-col self-start overflow-hidden p-4 sm:p-5 lg:max-h-full">
      <div className="mb-4 flex items-end justify-between">
        <div>
          <div className="mb-2 flex items-center gap-2">
            <span className="flex h-6 w-6 items-center justify-center rounded-full bg-gray-900 text-[11px] font-semibold text-white dark:bg-white dark:text-gray-900">
              1
            </span>
            <span className="rounded-full border border-border px-3 py-1 text-xs font-medium text-muted-foreground">Queue</span>
          </div>
          <h2 className="text-lg font-medium">Active jobs</h2>
        </div>
        <span className="text-xs text-muted-foreground">
          {live.length} running{waiting.length > 0 ? ` · ${waiting.length} waiting` : ""}
        </span>
      </div>
      <div className="scroll-thin min-h-0 flex-1 space-y-3 overflow-y-auto pr-1">
        {live.length === 0 && waiting.length === 0 && failed.length === 0 ? (
          <p className="text-sm text-muted-foreground">
            Downloads, decode, inference, and export show up here with live progress.
          </p>
        ) : (
          <>
            {waiting.length > 0 && live.length === 0 ? (
              <article className="rounded-xl border border-border bg-muted p-3">
                <p className="text-sm font-medium">Waiting in line</p>
                <p className="mt-1 text-xs text-muted-foreground">
                  {waiting.length} track{waiting.length === 1 ? "" : "s"} will process one at a time.
                </p>
              </article>
            ) : null}
            {waiting.length > 0 && live.length > 0 ? (
              <p className="text-xs text-muted-foreground">{waiting.length} more waiting. Only one track runs at a time.</p>
            ) : null}
            {live.map((track) => {
              const job = jobs[track.id];
              const eta = job ? formatEta(parseEtaSeconds(job)) : "";
              return (
                <article key={track.id} className="rounded-xl border border-border bg-muted p-3">
                  <div className="flex items-start justify-between gap-2">
                    <div className="min-w-0">
                      <p className="truncate text-sm font-medium">{track.title}</p>
                      <p className="truncate text-xs text-muted-foreground">{track.artist}</p>
                    </div>
                    <Loader2 className="h-4 w-4 shrink-0 animate-spin text-primary" />
                  </div>
                  <StageChips stage={job?.stage ?? track.status} />
                  <div className="mt-2 flex items-center justify-between gap-2 text-xs text-muted-foreground">
                    <p className="truncate">{job?.message ?? track.status}</p>
                    {eta ? <span className="shrink-0 font-medium text-primary">{eta}</span> : null}
                  </div>
                  <Progress className="mt-2" value={job?.progress ?? 0.05} />
                </article>
              );
            })}
            {failedAll.length > failed.length ? (
              <p className="text-xs text-muted-foreground">{failedAll.length - failed.length} more failed tracks are in the library.</p>
            ) : null}
            {failed.map((track) => (
              <article key={track.id} className="rounded-xl border border-red-300 bg-red-50 p-3 dark:border-red-900 dark:bg-red-950/40">
                <div className="flex items-start justify-between gap-2">
                  <p className="min-w-0 truncate text-sm font-medium">{track.title}</p>
                  <Button
                    size="icon"
                    variant="ghost"
                    className="h-11 w-11 shrink-0 text-red-700 hover:bg-red-100 dark:text-red-300 dark:hover:bg-red-900/40"
                    aria-label={`Dismiss error for ${track.title}`}
                    onClick={() => setHiddenErrors((ids) => (ids.includes(track.id) ? ids : [...ids, track.id]))}
                  >
                    <X className="h-4 w-4" />
                  </Button>
                </div>
                <p className="mt-1 text-xs text-red-700 dark:text-red-300">{track.error ?? "Failed"}</p>
                <Button
                  size="sm"
                  variant="outline"
                  className="mt-2"
                  onClick={async () => {
                    setHiddenErrors((ids) => ids.filter((id) => id !== track.id));
                    applyTrack({ ...track, status: "queued", error: null });
                    await retryTrack(track.id);
                  }}
                >
                  <RotateCcw className="h-3.5 w-3.5" />
                  Retry
                </Button>
              </article>
            ))}
          </>
        )}
      </div>
    </section>
  );
}
