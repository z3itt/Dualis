import { fileSrc } from "@/lib/api";
import { cn } from "@/lib/utils";

export function CoverArt({
  path,
  title,
  size = "md",
  className,
}: {
  path?: string | null;
  title: string;
  size?: "sm" | "md" | "lg";
  className?: string;
}) {
  const src = fileSrc(path);
  const hue = hashHue(title);
  return (
    <div
      className={cn(
        "relative shrink-0 overflow-hidden rounded-xl border border-border bg-muted",
        size === "sm" && "h-12 w-12",
        size === "md" && "h-16 w-16",
        size === "lg" && "h-16 w-16 xl:h-[4.5rem] xl:w-[4.5rem]",
        className
      )}
      style={src ? undefined : { background: `linear-gradient(145deg, hsl(${hue} 45% 88%), hsl(${hue} 35% 78%))` }}
      aria-hidden={!src}
    >
      {src ? (
        <img src={src} alt="" className="h-full w-full object-cover" />
      ) : (
        <span className="absolute inset-0 flex items-center justify-center text-lg font-semibold text-muted-foreground">
          {title.slice(0, 1).toUpperCase()}
        </span>
      )}
    </div>
  );
}

function hashHue(value: string) {
  let hash = 0;
  for (const char of value) {
    hash = (hash * 31 + char.charCodeAt(0)) % 360;
  }
  return hash;
}
