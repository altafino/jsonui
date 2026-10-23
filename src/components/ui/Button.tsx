import clsx from "clsx";
import type { ButtonHTMLAttributes, Ref } from "react";

type Variant = "primary" | "secondary" | "ghost" | "danger";

const BASE = "inline-flex items-center justify-center gap-2 rounded-md px-3 h-9 text-[0.9375rem] font-medium select-none transition-colors disabled:opacity-50 disabled:cursor-not-allowed";
const VARIANTS: Record<Variant, string> = {
  primary: "bg-primary text-on-primary hover:bg-primary-hover border border-transparent",
  secondary: "bg-transparent text-fg border border-ctl hover:bg-hover",
  ghost: "bg-transparent text-fg border border-transparent hover:bg-hover",
  danger: "bg-transparent text-danger border border-danger hover:bg-hover",
};

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: Variant;
  ref?: Ref<HTMLButtonElement>;
}

export function Button({ variant = "secondary", className, type = "button", ...rest }: ButtonProps) {
  return <button type={type} className={clsx(BASE, VARIANTS[variant], className)} {...rest} />;
}

export function IconButton({ label, className, children, ...rest }: ButtonProps & { label: string }) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      className={clsx("inline-flex items-center justify-center size-9 rounded-md border border-ctl text-fg hover:bg-hover disabled:opacity-50 disabled:cursor-not-allowed", className)}
      {...rest}
    >
      {children}
    </button>
  );
}
