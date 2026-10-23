import clsx from "clsx";
import { CircleAlert, Check, TriangleAlert } from "lucide-react";
import type { ReactNode } from "react";

/** Neutral meta chip (JSON, scope, Sample). */
export function Chip({ children, className, title }: { children: ReactNode; className?: string; title?: string }) {
  return (
    <span title={title} className={clsx("inline-flex items-center rounded border border-line px-1.5 h-5 text-xs mono text-muted whitespace-nowrap", className)}>
      {children}
    </span>
  );
}

type Tone = "ok" | "warn" | "error";
const TONES: Record<Tone, { cls: string; Icon: typeof Check }> = {
  ok: { cls: "text-ok border-ok/50", Icon: Check },
  warn: { cls: "text-warn border-warn/50", Icon: TriangleAlert },
  error: { cls: "text-danger border-danger/50", Icon: CircleAlert },
};

/** Status is never color alone (DD11): icon + text. */
export function StatusChip({ tone, children }: { tone: Tone; children: ReactNode }) {
  const { cls, Icon } = TONES[tone];
  return (
    <span className={clsx("inline-flex items-center gap-1 rounded border px-1.5 h-5 text-xs mono whitespace-nowrap", cls)}>
      <Icon className="size-3" aria-hidden />
      {children}
    </span>
  );
}

export function ChangedDot({ className }: { className?: string }) {
  return <span aria-hidden className={clsx("inline-block size-2 rounded-full bg-accent-fg shrink-0", className)} />;
}
