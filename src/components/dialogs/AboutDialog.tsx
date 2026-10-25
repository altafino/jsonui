import type { ReactNode } from "react";
import { APP_VERSION, AUTHOR, SITE_LABEL, TAGLINE } from "../../lib/about";
import { useApp } from "../../lib/store/app-store";
import { api, toCoreError } from "../../lib/tauri-api";
import { Button } from "../ui/Button";
import { Dialog } from "../ui/Dialog";

function Fact({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="border-b border-line py-3">
      <p className="text-xs font-medium uppercase tracking-wider text-muted">{label}</p>
      <div className="mt-1">{children}</div>
    </div>
  );
}

/** About: version, author, and the two links the app is allowed to open. */
export function AboutDialog() {
  const close = useApp((s) => s.closeDialog);
  const announce = useApp((s) => s.announce);
  const open = async (which: "site" | "releases") => {
    try {
      await api.openAboutLink(which);
    } catch (e) {
      announce(toCoreError(e).message, "assertive");
    }
  };
  return (
    <Dialog
      open
      onClose={close}
      title="About"
      description={TAGLINE}
      footer={<Button onClick={close}>Close</Button>}
    >
      <p className="text-xl font-semibold tracking-tight">ConfigDeck</p>
      <p className="mt-1 font-mono text-[0.8125rem] text-muted">Version {APP_VERSION}</p>
      <dl className="mt-4">
        <Fact label="Release">
          <p>ConfigDeck {APP_VERSION}</p>
          <button type="button" onClick={() => void open("releases")} className="mt-1 text-accent-fg underline underline-offset-2 hover:no-underline">
            Release notes
          </button>
        </Fact>
        <Fact label="Author">
          <p>{AUTHOR}</p>
        </Fact>
        <Fact label="Website">
          <button type="button" onClick={() => void open("site")} className="text-accent-fg underline underline-offset-2 hover:no-underline">
            {SITE_LABEL}
          </button>
        </Fact>
        <Fact label="License">
          <p>Apache License 2.0</p>
        </Fact>
      </dl>
    </Dialog>
  );
}
