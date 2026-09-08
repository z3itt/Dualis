import { cn } from "@/lib/utils";

export function BrandMark({
  theme,
  className,
}: {
  theme: "light" | "dark";
  className?: string;
}) {
  const src = theme === "dark" ? "/brand/dualis-dark.jpg" : "/brand/dualis-light.jpg";
  return (
    <span className={cn("squircle inline-flex h-14 w-14 shrink-0 bg-card", className)}>
      <img src={src} alt="DUΛLIS" width={56} height={56} className="h-full w-full object-contain" />
    </span>
  );
}
