import * as CM from "@radix-ui/react-context-menu";
import * as DM from "@radix-ui/react-dropdown-menu";
import clsx from "clsx";
import type { ReactNode } from "react";

const CONTENT = "cd-popover z-[60] min-w-48 rounded-md border border-line bg-sidebar p-1 text-sm text-fg";
const ITEM = "flex items-center gap-2 rounded px-2.5 h-8 cursor-default select-none outline-none data-[highlighted]:bg-hover data-[disabled]:opacity-50";

export interface MenuEntry {
  label: string;
  onSelect: () => void;
  icon?: ReactNode;
  disabled?: boolean;
  danger?: boolean;
  separatorBefore?: boolean;
}

export function DropdownMenu({ trigger, entries, align = "end", label }: { trigger: ReactNode; entries: MenuEntry[]; align?: "start" | "end"; label?: string }) {
  return (
    <DM.Root>
      <DM.Trigger asChild aria-label={label}>{trigger}</DM.Trigger>
      <DM.Portal>
        <DM.Content align={align} sideOffset={4} className={CONTENT}>
          {entries.map((e) => (
            <div key={e.label}>
              {e.separatorBefore ? <DM.Separator className="my-1 h-px bg-[var(--border)]" /> : null}
              <DM.Item disabled={e.disabled} onSelect={e.onSelect} className={clsx(ITEM, e.danger && "text-danger")}>
                {e.icon}
                {e.label}
              </DM.Item>
            </div>
          ))}
        </DM.Content>
      </DM.Portal>
    </DM.Root>
  );
}

/** Right-click or Shift+F10 / the Menu key opens it (Radix handles keyboard). */
export function ContextMenu({ children, entries }: { children: ReactNode; entries: MenuEntry[] }) {
  return (
    <CM.Root>
      <CM.Trigger asChild>{children}</CM.Trigger>
      <CM.Portal>
        <CM.Content className={CONTENT}>
          {entries.map((e) => (
            <div key={e.label}>
              {e.separatorBefore ? <CM.Separator className="my-1 h-px bg-[var(--border)]" /> : null}
              <CM.Item disabled={e.disabled} onSelect={e.onSelect} className={clsx(ITEM, e.danger && "text-danger")}>
                {e.icon}
                {e.label}
              </CM.Item>
            </div>
          ))}
        </CM.Content>
      </CM.Portal>
    </CM.Root>
  );
}
