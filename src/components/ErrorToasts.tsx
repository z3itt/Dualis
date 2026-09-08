import { X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useAppStore } from "@/store/app";

export function ErrorToasts() {
  const errors = useAppStore((s) => s.errors);
  const dismissError = useAppStore((s) => s.dismissError);
  const clearErrors = useAppStore((s) => s.clearErrors);
  if (errors.length === 0) {
    return null;
  }
  const latest = errors[0];
  return (
    <div className="pointer-events-none fixed right-4 top-20 z-30 flex w-[min(360px,calc(100%-2rem))] flex-col gap-2">
      <div className="pointer-events-auto surface-raised rounded-2xl p-4">
        <div className="flex items-start justify-between gap-3">
          <div>
            <p className="text-xs font-medium uppercase tracking-wide text-destructive">Job failed</p>
            <p className="mt-1 text-sm text-foreground">{latest.message}</p>
          </div>
          <Button size="icon" variant="ghost" aria-label="Dismiss error" onClick={() => dismissError(latest.id)}>
            <X className="h-4 w-4" />
          </Button>
        </div>
        {errors.length > 1 ? (
          <button type="button" className="mt-2 text-xs text-muted-foreground hover:text-foreground" onClick={clearErrors}>
            Clear {errors.length} errors
          </button>
        ) : null}
      </div>
    </div>
  );
}
