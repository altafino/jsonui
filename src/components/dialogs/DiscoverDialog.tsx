import { useEffect, useState } from "react";
import { addDiscovered } from "../../lib/store/actions";
import { useApp } from "../../lib/store/app-store";
import { api, toCoreError } from "../../lib/tauri-api";
import type { DiscoveredCandidate } from "../../lib/types/generated/DiscoveredCandidate";
import { R1_HINT } from "../document/Banners";
import { SCOPE_LABEL } from "../document/DocumentHeader";
import { Button } from "../ui/Button";
import { Chip } from "../ui/Chip";
import { Dialog } from "../ui/Dialog";

function Candidate({ c, onAdd }: { c: DiscoveredCandidate; onAdd: () => void }) {
  return (
    <li className="flex items-center gap-3 py-2.5" data-testid="candidate">
      <div className="min-w-0 flex-1">
        <p className="flex flex-wrap items-center gap-2">
          <Chip>{SCOPE_LABEL[c.scope]}</Chip>
          <span className="mono truncate text-[0.9375rem]">{c.path}</span>
        </p>
        <p className="mt-0.5 text-sm text-muted">
          {c.alreadyRegistered ? "Already in ConfigDeck." : !c.exists ? "File does not exist yet." : c.readOnly ? "Read-only: ConfigDeck will not change it." : ""} {c.note ?? ""}
        </p>
      </div>
      <Button onClick={onAdd} disabled={c.alreadyRegistered || (!c.exists && c.scope === "managed")} aria-label={`${c.exists ? "Add" : "Create"} ${c.path}`}>
        {c.alreadyRegistered ? "Added" : c.exists ? "Add" : "Create file"}
      </Button>
    </li>
  );
}

/** Claude Code discovery: user and managed candidates, plus project folders picked through Rust (eng R6). */
export function DiscoverDialog() {
  const close = useApp((s) => s.closeDialog);
  const [list, setList] = useState<DiscoveredCandidate[] | null>(null);
  const [project, setProject] = useState<DiscoveredCandidate[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api.listDiscoverableClaudeFiles().then(setList, (e) => setError(toCoreError(e).message));
  }, []);

  const add = async (c: DiscoveredCandidate) => {
    await addDiscovered(c.candidateId);
    close();
  };
  const choose = async () => {
    try {
      const r = await api.pickProjectDirectory();
      if (r) setProject(r);
    } catch (e) {
      setError(toCoreError(e).message);
    }
  };

  const user = list?.find((c) => c.scope === "user");
  return (
    <Dialog open onClose={close} title="Discover Claude Code settings" description="Add the settings files Claude Code uses. Nothing is changed until you save." wide footer={<Button onClick={close}>Close</Button>}>
      {error ? <p role="alert" className="mb-3 text-danger">{error}</p> : null}
      {user && !user.exists && !user.alreadyRegistered ? (
        <div className="mb-3 rounded-md border border-line px-3 py-3" data-testid="no-user-settings">
          <p>No user settings at <span className="mono">{user.path}</span></p>
          <div className="mt-2 flex gap-2">
            <Button onClick={() => void add(user)}>Create file</Button>
            <Button onClick={() => void choose()}>Choose project folder</Button>
          </div>
        </div>
      ) : null}
      <ul className="divide-y divide-[var(--border)]">
        {(list ?? []).filter((c) => c.exists || c.alreadyRegistered).map((c) => (
          <Candidate key={c.candidateId} c={c} onAdd={() => void add(c)} />
        ))}
      </ul>
      {project ? (
        <section aria-label="Project settings" className="mt-4">
          <h3 className="text-sm font-medium">Project folder</h3>
          <ul className="divide-y divide-[var(--border)]">
            {project.map((c) => (
              <Candidate key={c.candidateId} c={c} onAdd={() => void add(c)} />
            ))}
          </ul>
          <p className="mt-2 text-sm text-muted">{R1_HINT}</p>
        </section>
      ) : (
        <div className="mt-3">
          <Button onClick={() => void choose()}>Choose project folder…</Button>
        </div>
      )}
    </Dialog>
  );
}
