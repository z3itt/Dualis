import { cn } from "@/lib/utils";
import { STAGES, stageIndex } from "@/lib/library";

const LABELS: Record<(typeof STAGES)[number], string> = {
  download: "Download",
  decode: "Decode",
  infer: "Infer",
  export: "Export",
};

export function StageChips({ stage }: { stage?: string }) {
  const active = stageIndex(stage ?? "download");
  return (
    <div className="mt-2 flex flex-wrap gap-1.5">
      {STAGES.map((item, index) => {
        const on = index <= active;
        const current = index === active;
        return (
          <span
            key={item}
            className={cn(
              "rounded-full px-2 py-0.5 text-[10px] font-medium uppercase tracking-wide transition-colors duration-200",
              current && "bg-primary text-primary-foreground",
              on && !current && "bg-secondary text-foreground",
              !on && "bg-muted text-muted-foreground"
            )}
          >
            {LABELS[item]}
          </span>
        );
      })}
    </div>
  );
}
