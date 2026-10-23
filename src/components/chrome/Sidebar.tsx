import { useRef } from "react";
import { useApp } from "../../lib/store/app-store";
import { FileList } from "../file-list/FileList";

const MIN = 220;
const MAX = 480;
const RAIL = 56;

/**
 * Wide screens: resizable column (or a 56 px rail when collapsed).
 * Narrow screens (< 1180 px): a permanent rail; opening it shows the full list as an overlay (DD12).
 */
export function Sidebar() {
  const { rail: narrow, sidebarOpen, prefs, setPrefs, toggleSidebar } = useApp();
  const drag = useRef<{ x: number; w: number } | null>(null);
  const expanded = sidebarOpen;
  const showRail = narrow || !expanded;
  const width = showRail ? RAIL : prefs.sidebarWidth;
  const overlay = narrow && expanded;

  const resize = (w: number) => void setPrefs({ sidebarWidth: Math.max(MIN, Math.min(MAX, Math.round(w))) });

  return (
    <>
      <aside style={{ width }} className="relative shrink-0 border-r border-line" data-testid="sidebar" data-mode={showRail ? "rail" : "full"}>
        <FileList rail={showRail} />
        {!showRail ? (
          <div
            role="separator"
            aria-orientation="vertical"
            aria-label="Resize sidebar"
            aria-valuenow={prefs.sidebarWidth}
            aria-valuemin={MIN}
            aria-valuemax={MAX}
            tabIndex={0}
            onPointerDown={(e) => {
              (e.target as HTMLElement).setPointerCapture(e.pointerId);
              drag.current = { x: e.clientX, w: prefs.sidebarWidth };
            }}
            onPointerMove={(e) => drag.current && resize(drag.current.w + e.clientX - drag.current.x)}
            onPointerUp={() => (drag.current = null)}
            onKeyDown={(e) => {
              if (e.key === "ArrowLeft") (e.preventDefault(), resize(prefs.sidebarWidth - 16));
              if (e.key === "ArrowRight") (e.preventDefault(), resize(prefs.sidebarWidth + 16));
            }}
            className="absolute inset-y-0 -right-1 z-10 w-2 cursor-col-resize hover:bg-accent/40 focus-visible:bg-accent/60"
          />
        ) : null}
      </aside>
      {overlay ? (
        <>
          <div className="fixed inset-0 top-14 z-20" onClick={toggleSidebar} aria-hidden />
          <aside style={{ width: prefs.sidebarWidth }} className="cd-popover fixed bottom-0 left-0 top-14 z-30 border-r border-line bg-sidebar" data-testid="sidebar-overlay" aria-label="Expanded sidebar">
            <FileList rail={false} />
          </aside>
        </>
      ) : null}
    </>
  );
}
