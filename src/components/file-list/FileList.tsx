import { ChevronDown, ChevronRight, FilePlus, FlaskConical, FolderSearch, Plus, FileText } from "lucide-react";
import { useMemo, useState } from "react";
import { addFile, createFile, trySample } from "../../lib/store/actions";
import { useApp } from "../../lib/store/app-store";
import { Button, IconButton } from "../ui/Button";
import { DropdownMenu } from "../ui/Menu";
import { FileRow } from "./FileRow";
import { groupFiles } from "./groups";

/** Navigation landmark. `rail` renders the 56 px icon rail (below 1180 px or collapsed). */
export function FileList({ rail }: { rail: boolean }) {
  const files = useApp((s) => s.files);
  const openDialog = useApp((s) => s.openDialog);
  const groups = useMemo(() => groupFiles(files), [files]);
  const [collapsed, setCollapsed] = useState<Record<string, boolean>>({});
  const addEntries = [
    { label: "Add JSON file…", icon: <FilePlus className="size-4" aria-hidden />, onSelect: () => void addFile() },
    { label: "Create new JSON file…", icon: <FileText className="size-4" aria-hidden />, onSelect: () => void createFile() },
    { label: "Discover Claude Code settings", icon: <FolderSearch className="size-4" aria-hidden />, onSelect: () => openDialog({ kind: "discover" }) },
    { label: "Try with a sample", icon: <FlaskConical className="size-4" aria-hidden />, onSelect: () => void trySample() },
  ];

  return (
    <nav data-landmark="navigation" tabIndex={-1} aria-label="Configurations" className="flex h-full min-h-0 flex-col bg-sidebar">
      {rail ? (
        <div className="flex justify-center py-3">
          <DropdownMenu align="start" label="Add file" trigger={<IconButton label="Add file"><Plus className="size-4" aria-hidden /></IconButton>} entries={addEntries} />
        </div>
      ) : (
        <div className="px-4 pb-3 pt-4">
          <h2 className="mb-3 text-xs font-medium uppercase tracking-[0.14em] text-muted">Configurations</h2>
          <DropdownMenu align="start" label="Add file" trigger={<Button className="w-full"><Plus className="size-4" aria-hidden /> Add file</Button>} entries={addEntries} />
        </div>
      )}
      <div className="min-h-0 flex-1 overflow-y-auto overflow-x-hidden pb-4">
        {groups.map((g) => {
          const closed = !!collapsed[g.id] && !rail;
          return (
            <section key={g.id} aria-label={g.title}>
              {!rail ? (
                <button
                  type="button"
                  aria-expanded={!closed}
                  onClick={() => setCollapsed((c) => ({ ...c, [g.id]: !c[g.id] }))}
                  className="flex w-full items-center justify-between px-4 py-2.5 text-left text-[1.0625rem] font-medium hover:bg-hover"
                >
                  {g.title}
                  {closed ? <ChevronRight className="size-4 text-muted" aria-hidden /> : <ChevronDown className="size-4 text-muted" aria-hidden />}
                </button>
              ) : (
                <div className="mx-2 my-1 h-px bg-[var(--border)]" />
              )}
              {!closed ? (
                <ul>
                  {g.files.map((f) => (
                    <FileRow key={f.id} file={f} rail={rail} />
                  ))}
                </ul>
              ) : null}
            </section>
          );
        })}
      </div>
    </nav>
  );
}
