import * as RT from "@radix-ui/react-tooltip";
import type { ReactNode } from "react";

export const TooltipProvider = ({ children }: { children: ReactNode }) => <RT.Provider delayDuration={400}>{children}</RT.Provider>;

/** Wraps any focusable child; `content` null renders the child untouched. */
export function Tip({ content, children, side = "top" }: { content: ReactNode | null | undefined; children: ReactNode; side?: "top" | "bottom" | "left" | "right" }) {
  // Always the same tree shape so toggling `content` never remounts (and unfocuses) the child.
  return (
    <RT.Root open={content ? undefined : false}>
      <RT.Trigger asChild>{children}</RT.Trigger>
      <RT.Portal>
        <RT.Content side={side} sideOffset={6} className="cd-popover z-[60] max-w-xs rounded-md border border-line bg-sidebar px-2.5 py-1.5 text-sm text-fg">
          {content}
        </RT.Content>
      </RT.Portal>
    </RT.Root>
  );
}
