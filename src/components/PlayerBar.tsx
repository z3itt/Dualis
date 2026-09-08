import { ListMusic, Pause, Play, Repeat, Repeat1, Shuffle, SkipBack, SkipForward, Volume2, X } from "lucide-react";
import { cn } from "@/lib/utils";
import { CoverArt } from "@/components/CoverArt";
import { StemModeToggle } from "@/components/StemModeToggle";
import { Waveform } from "@/components/Waveform";
import { Button } from "@/components/ui/button";
import { formatTime } from "@/lib/utils";
import { useAppStore } from "@/store/app";

export function PlayerBar() {
  const tracks = useAppStore((s) => s.tracks);
  const currentId = useAppStore((s) => s.currentId);
  const playing = useAppStore((s) => s.playing);
  const currentTime = useAppStore((s) => s.currentTime);
  const duration = useAppStore((s) => s.duration);
  const volume = useAppStore((s) => s.volume);
  const mode = useAppStore((s) => s.mode);
  const waveform = useAppStore((s) => s.waveform);
  const queue = useAppStore((s) => s.queue);
  const queueOpen = useAppStore((s) => s.queueOpen);
  const shuffle = useAppStore((s) => s.shuffle);
  const loopMode = useAppStore((s) => s.loopMode);
  const togglePlay = useAppStore((s) => s.togglePlay);
  const seek = useAppStore((s) => s.seek);
  const setVolume = useAppStore((s) => s.setVolume);
  const setMode = useAppStore((s) => s.setMode);
  const setShuffle = useAppStore((s) => s.setShuffle);
  const cycleLoopMode = useAppStore((s) => s.cycleLoopMode);
  const skip = useAppStore((s) => s.skip);
  const setQueueOpen = useAppStore((s) => s.setQueueOpen);
  const playTrack = useAppStore((s) => s.playTrack);
  const removeFromQueue = useAppStore((s) => s.removeFromQueue);
  const clearQueue = useAppStore((s) => s.clearQueue);

  const current = tracks.find((track) => track.id === currentId);
  const queued = queue
    .map((id) => tracks.find((track) => track.id === id))
    .filter((track): track is NonNullable<typeof track> => Boolean(track));

  return (
    <footer className="relative">
      {queueOpen ? (
        <div className="surface-raised absolute bottom-[calc(100%+12px)] right-0 z-20 w-full max-w-md rounded-2xl p-4">
          <div className="mb-3 flex items-center justify-between">
            <h3 className="text-base font-medium">Play queue</h3>
            <div className="flex gap-2">
              <Button size="sm" variant="ghost" onClick={clearQueue}>
                Clear
              </Button>
              <Button size="icon" variant="ghost" aria-label="Close queue" onClick={() => setQueueOpen(false)}>
                <X className="h-4 w-4" />
              </Button>
            </div>
          </div>
          <ul className="scroll-thin max-h-64 space-y-2 overflow-y-auto">
            {queued.length === 0 ? (
              <li className="text-sm text-muted-foreground">Queue is empty. Add ready tracks from the library.</li>
            ) : (
              queued.map((track, index) => (
                <li key={track.id} className="flex items-center gap-3 rounded-xl bg-muted px-2 py-2">
                  <span className="w-5 text-xs text-muted-foreground">{index + 1}</span>
                  <CoverArt path={track.coverPath} title={track.title} size="sm" className="h-10 w-10 rounded-lg" />
                  <button type="button" className="min-w-0 flex-1 text-left" onClick={() => playTrack(track)}>
                    <p className="truncate text-sm">{track.title}</p>
                    <p className="truncate text-xs text-muted-foreground">{track.artist}</p>
                  </button>
                  <Button size="icon" variant="ghost" aria-label={`Remove ${track.title}`} onClick={() => removeFromQueue(track.id)}>
                    <X className="h-4 w-4" />
                  </Button>
                </li>
              ))
            )}
          </ul>
        </div>
      ) : null}
      <div className="surface-raised rounded-2xl p-4">
        <div className="grid gap-4 xl:grid-cols-[240px_1fr_auto] xl:items-center">
          <div className="flex min-w-0 items-center gap-3">
            <CoverArt path={current?.coverPath} title={current?.title ?? "Dualis"} size="lg" />
            <div className="min-w-0">
              <p className="truncate text-base font-medium leading-tight">{current?.title ?? "Nothing playing"}</p>
              <p className="truncate text-xs text-muted-foreground">{current?.artist ?? "Load a ready stem to begin"}</p>
            </div>
          </div>
          <div className="space-y-2">
            <div className="flex flex-wrap items-center justify-center gap-2">
              <Button
                size="icon"
                variant="ghost"
                aria-label={shuffle ? "Disable shuffle" : "Enable shuffle"}
                aria-pressed={shuffle}
                title={shuffle ? "Shuffle on" : "Shuffle off"}
                onClick={() => setShuffle(!shuffle)}
                className={cn(shuffle ? "text-primary" : "text-muted-foreground")}
              >
                <Shuffle className="h-5 w-5" />
              </Button>
              <Button size="icon" variant="ghost" aria-label="Previous track" onClick={() => void skip(-1)}>
                <SkipBack className="h-5 w-5" />
              </Button>
              <Button size="icon" variant="solid" aria-label={playing ? "Pause" : "Play"} onClick={() => void togglePlay()}>
                {playing ? <Pause className="h-5 w-5 fill-current" /> : <Play className="h-5 w-5 fill-current" />}
              </Button>
              <Button size="icon" variant="ghost" aria-label="Next track" onClick={() => void skip(1)}>
                <SkipForward className="h-5 w-5" />
              </Button>
              <Button
                size="icon"
                variant="ghost"
                aria-label={
                  loopMode === "song" ? "Repeat song" : loopMode === "queue" ? "Repeat queue" : "Repeat off"
                }
                aria-pressed={loopMode !== "off"}
                title={
                  loopMode === "song" ? "Repeating this song" : loopMode === "queue" ? "Repeating the queue" : "Repeat off"
                }
                onClick={cycleLoopMode}
                className={cn(loopMode !== "off" ? "text-primary" : "text-muted-foreground")}
              >
                {loopMode === "song" ? <Repeat1 className="h-5 w-5" /> : <Repeat className="h-5 w-5" />}
              </Button>
              <StemModeToggle value={mode} onChange={setMode} />
            </div>
            <div className="flex items-center gap-3">
              <span className="w-10 text-right text-xs text-muted-foreground">{formatTime(currentTime)}</span>
              <Waveform
                bars={waveform}
                progress={duration > 0 ? currentTime / duration : 0}
                onSeek={(ratio) => seek(ratio * duration)}
              />
              <span className="w-10 text-xs text-muted-foreground">{formatTime(duration)}</span>
            </div>
          </div>
          <div className="flex items-center gap-3 xl:justify-end">
            <Button
              variant={queueOpen ? "solid" : "outline"}
              size="sm"
              aria-pressed={queueOpen}
              onClick={() => setQueueOpen(!queueOpen)}
            >
              <ListMusic className="h-4 w-4" />
              Queue {queue.length > 0 ? queue.length : ""}
            </Button>
            <label className="flex items-center gap-2">
              <Volume2 className="h-4 w-4 text-muted-foreground" />
              <span className="sr-only">Master volume</span>
              <input
                type="range"
                min={0}
                max={1}
                step={0.01}
                value={volume}
                onChange={(event) => setVolume(Number(event.target.value))}
                className="h-11 w-28 accent-primary"
              />
            </label>
          </div>
        </div>
      </div>
    </footer>
  );
}
