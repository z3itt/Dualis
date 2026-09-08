import type { StemMode } from "@/lib/types";
import { cn } from "@/lib/utils";

const modes: { id: StemMode; label: string }[] = [
  { id: "original", label: "Normal" },
  { id: "vocals", label: "Vocal" },
  { id: "instrumental", label: "Inst" },
];

export function StemModeToggle({
  value,
  onChange,
}: {
  value: StemMode;
  onChange: (mode: StemMode) => void;
}) {
  return (
    <div
      role="radiogroup"
      aria-label="Stem mode"
      className="inline-flex rounded-full border border-border bg-muted p-1"
    >
      {modes.map((mode) => {
        const active = value === mode.id;
        return (
          <button
            key={mode.id}
            type="button"
            role="radio"
            aria-checked={active}
            onClick={() => onChange(mode.id)}
            className={cn(
              "h-8 min-w-14 rounded-full px-3 text-xs transition-colors duration-200",
              active ? "bg-gray-900 text-white dark:bg-white dark:text-gray-900" : "text-muted-foreground hover:text-foreground"
            )}
          >
            {mode.label}
          </button>
        );
      })}
    </div>
  );
}
