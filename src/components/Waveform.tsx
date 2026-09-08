import { useMemo } from "react";

export function Waveform({
  bars,
  progress,
  onSeek,
}: {
  bars: number[];
  progress: number;
  onSeek: (ratio: number) => void;
}) {
  const values = useMemo(() => (bars.length > 0 ? bars : Array.from({ length: 64 }, () => 0.12)), [bars]);

  return (
    <div
      className="flex h-14 w-full cursor-pointer items-center gap-[2px]"
      role="slider"
      aria-label="Seek"
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={Math.round(progress * 100)}
      tabIndex={0}
      onClick={(event) => {
        const rect = event.currentTarget.getBoundingClientRect();
        onSeek((event.clientX - rect.left) / rect.width);
      }}
      onKeyDown={(event) => {
        if (event.key === "ArrowRight") {
          onSeek(Math.min(1, progress + 0.02));
        }
        if (event.key === "ArrowLeft") {
          onSeek(Math.max(0, progress - 0.02));
        }
      }}
    >
      {values.map((value, index) => {
        const filled = index / values.length <= progress;
        return (
          <div
            key={index}
            className={`w-full rounded-full transition-colors duration-150 ${filled ? "bg-primary" : "bg-border"}`}
            style={{ height: `${Math.max(14, value * 100)}%` }}
          />
        );
      })}
    </div>
  );
}
