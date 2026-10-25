import { PanelLeft, Search, Settings } from "lucide-react";
import { formatKeys } from "../../lib/shortcuts";
import { useApp } from "../../lib/store/app-store";
import { IconButton } from "../ui/Button";

/** Banner landmark: sidebar toggle, wordmark, command search (this file only; workspace search is P1), settings. */
export function TitleBar() {
  const openDialog = useApp((s) => s.openDialog);
  const toggleSidebar = useApp((s) => s.toggleSidebar);
  const active = useApp((s) => s.activeFileId);
  return (
    <header data-landmark="banner" tabIndex={-1} role="banner" className="flex h-14 shrink-0 items-center gap-3 border-b border-line bg-bg px-3">
      <IconButton label="Toggle sidebar" className="border-transparent" onClick={toggleSidebar}>
        <PanelLeft className="size-5" aria-hidden />
      </IconButton>
      <span className="text-[1.0625rem] font-semibold tracking-tight">ConfigDeck</span>
      <div className="mx-auto w-full max-w-xl">
        {active ? <button
          type="button"
          onClick={() => openDialog({ kind: "command" })}
          className="flex h-9 w-full items-center gap-2 rounded-md border border-ctl px-3 text-left text-[0.9375rem] text-muted hover:bg-hover disabled:opacity-60"
          aria-label="Go to setting"
          aria-keyshortcuts="Control+K Meta+K"
        >
          <Search className="size-4" aria-hidden />
          <span className="flex-1 truncate">Go to setting</span>
          <kbd className="mono text-[0.8125rem]">{formatKeys("Mod+K")}</kbd>
        </button> : null}
      </div>
      <IconButton label="Settings" className="border-transparent" onClick={() => openDialog({ kind: "settings" })}>
        <Settings className="size-5" aria-hidden />
      </IconButton>
    </header>
  );
}
