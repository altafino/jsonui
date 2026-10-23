import { useEffect } from "react";
import { SaveBar } from "./components/chrome/SaveBar";
import { Sidebar } from "./components/chrome/Sidebar";
import { TitleBar } from "./components/chrome/TitleBar";
import { DialogHost } from "./components/dialogs/DialogHost";
import { DocumentView } from "./components/document/DocumentView";
import { Welcome } from "./components/document/EmptyStates";
import { Button } from "./components/ui/Button";
import { TooltipProvider } from "./components/ui/Tooltip";
import { usePrefsEffects } from "./lib/hooks/use-prefs-effects";
import { useShortcuts } from "./lib/hooks/use-shortcuts";
import { bootstrap } from "./lib/store/actions";
import { useApp } from "./lib/store/app-store";

function Notices() {
  const notices = useApp((s) => s.notices);
  const dismissed = useApp((s) => s.dismissedNotices);
  const dismiss = useApp((s) => s.dismissNotice);
  const info = useApp((s) => s.info);
  const shown = notices.filter((n) => !dismissed.includes(n.code));
  if (!shown.length && !info) return null;
  return (
    <div className="border-b border-line">
      {shown.map((n) => (
        <div key={n.code} role="status" className="flex items-center gap-3 px-4 py-2 text-sm" data-testid="startup-notice">
          <span className="min-w-0 flex-1">
            {n.message}
            {n.path ? <span className="mono ml-2 text-[0.8125rem] text-muted">{n.path}</span> : null}
          </span>
          <Button variant="ghost" className="h-7" onClick={() => dismiss(n.code)}>Dismiss</Button>
        </div>
      ))}
      {info ? <div role="status" className="px-4 py-2 text-sm">{info}</div> : null}
    </div>
  );
}

/** Polite and assertive live regions for save, conflict and copy messages. */
function LiveRegions() {
  const a = useApp((s) => s.announcement);
  return (
    <>
      <div className="sr-only-live" role="status" aria-live="polite" aria-atomic data-testid="live-polite" key={`p${a.tick}`}>{a.polite}</div>
      <div className="sr-only-live" role="alert" aria-live="assertive" aria-atomic data-testid="live-assertive" key={`a${a.tick}`}>{a.assertive}</div>
    </>
  );
}

export default function App() {
  const ready = useApp((s) => s.ready);
  const fatal = useApp((s) => s.fatal);
  const files = useApp((s) => s.files);
  const active = useApp((s) => s.activeFileId);
  usePrefsEffects();
  useShortcuts();
  useEffect(() => void bootstrap(), []);

  return (
    <TooltipProvider>
      <div className="flex h-full min-w-0 flex-col bg-bg text-fg">
        <TitleBar />
        <Notices />
        {fatal ? (
          <div role="alert" className="border-b border-danger/60 px-4 py-2 text-sm">
            {fatal.message} {fatal.remediation ?? ""} <span className="mono text-muted">{fatal.code}</span>
          </div>
        ) : null}
        <div className="flex min-h-0 flex-1">
          {ready && files.length > 0 ? <Sidebar /> : null}
          <main data-landmark="main" tabIndex={-1} aria-label="Document" className="min-w-0 flex-1 overflow-hidden">
            {!ready ? null : files.length === 0 || !active ? files.length === 0 ? <Welcome /> : <div className="grid h-full place-items-center text-muted">Select a file</div> : <DocumentView key={active} fileId={active} />}
          </main>
        </div>
        {ready && files.length > 0 && active ? <SaveBar /> : null}
        <DialogHost />
        <LiveRegions />
      </div>
    </TooltipProvider>
  );
}
