import { type ButtonHTMLAttributes, type ReactNode } from "react";
import { cn } from "@/lib/utils";

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: "primary" | "ghost" | "outline" | "danger" | "solid";
  size?: "md" | "icon" | "sm";
}

export function Button({
  className,
  variant = "primary",
  size = "md",
  type = "button",
  ...props
}: ButtonProps) {
  return (
    <button
      type={type}
      className={cn(
        "inline-flex items-center justify-center gap-2 rounded-full text-sm font-medium transition-colors duration-200 focus-visible:ring-2 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-40",
        variant === "primary" && "bg-primary text-primary-foreground hover:opacity-90",
        variant === "solid" && "bg-gray-900 text-white hover:bg-gray-800 dark:bg-white dark:text-gray-900 dark:hover:bg-gray-200",
        variant === "ghost" && "bg-transparent text-foreground hover:bg-muted",
        variant === "outline" && "border border-border bg-card text-foreground hover:bg-muted",
        variant === "danger" && "bg-destructive text-destructive-foreground hover:opacity-90",
        size === "md" && "h-10 min-w-10 px-4",
        size === "sm" && "h-8 min-w-8 px-3 text-xs",
        size === "icon" && "h-10 w-10",
        className
      )}
      {...props}
    />
  );
}

export function TextRollButton({
  children,
  className,
  icon,
  variant = "primary",
  ...props
}: ButtonProps & { icon?: ReactNode }) {
  return (
    <Button variant={variant} className={cn("group pr-1.5", className)} {...props}>
      <span className="relative flex h-5 overflow-hidden">
        <span className="flex flex-col transition-transform duration-500 ease-[cubic-bezier(0.25,0.1,0.25,1)] group-hover:-translate-y-1/2 motion-reduce:transform-none">
          <span>{children}</span>
          <span aria-hidden="true">{children}</span>
        </span>
      </span>
      <span className="flex h-7 w-7 items-center justify-center rounded-full bg-white/90 text-primary transition-transform duration-500 ease-[cubic-bezier(0.25,0.1,0.25,1)] group-hover:-rotate-45 motion-reduce:transform-none dark:bg-black/20 dark:text-white">
        {icon}
      </span>
    </Button>
  );
}
