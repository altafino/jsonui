import * as RD from "@radix-ui/react-dialog";
import clsx from "clsx";
import { X } from "lucide-react";
import type { ReactNode } from "react";

/** Modal dialog: min(720px, 90vw), 8px radius, shadow only here (DD9, DD12). Esc closes. */
export function Dialog({
  open, onClose, title, description, children, footer, wide, labelledByExtra, onCloseAutoFocus,
}: {
  open: boolean;
  onClose: () => void;
  title: string;
  description?: string;
  children: ReactNode;
  footer?: ReactNode;
  wide?: boolean;
  labelledByExtra?: string;
  /** When set, replaces Radix returning focus to the control that opened the dialog. */
  onCloseAutoFocus?: (event: Event) => void;
}) {
  return (
    <RD.Root open={open} onOpenChange={(o) => !o && onClose()}>
      <RD.Portal>
        <RD.Overlay className="fixed inset-0 z-40" style={{ background: "var(--overlay)" }} />
        <RD.Content
          aria-describedby={description ? undefined : undefined}
          onCloseAutoFocus={onCloseAutoFocus}
          data-testid="dialog"
          className={clsx(
            "cd-popover fixed z-50 left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2 flex flex-col max-h-[85vh] rounded-lg border border-line bg-sidebar text-fg",
            wide ? "w-[min(880px,92vw)]" : "w-[min(720px,90vw)]",
          )}
        >
          <div className="flex items-start gap-3 px-6 pt-5 pb-3">
            <div className="min-w-0 flex-1">
              <RD.Title className="text-xl font-semibold leading-tight">{title}</RD.Title>
              {description ? <RD.Description className="mt-1 text-sm text-muted">{description}</RD.Description> : <RD.Description className="sr-only">{labelledByExtra ?? title}</RD.Description>}
            </div>
            <RD.Close aria-label="Close" className="rounded-md p-1.5 text-muted hover:bg-hover hover:text-fg">
              <X className="size-4" aria-hidden />
            </RD.Close>
          </div>
          <div className="min-h-0 flex-1 overflow-auto px-6 pb-4">{children}</div>
          {footer ? <div className="flex flex-wrap items-center justify-end gap-2 border-t border-line px-6 py-3">{footer}</div> : null}
        </RD.Content>
      </RD.Portal>
    </RD.Root>
  );
}
