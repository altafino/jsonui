import { humanize } from "../../lib/json/pointer";
import type { ManagedDocument } from "../../lib/types/generated/ManagedDocument";

export interface FileGroup {
  id: string;
  title: string;
  files: ManagedDocument[];
}

/** Sidebar groups: Claude Code (preset/collection), Sample, user collections, then everything else. */
export function groupFiles(files: ManagedDocument[]): FileGroup[] {
  const groups = new Map<string, FileGroup>();
  const put = (id: string, title: string, f: ManagedDocument) => {
    const g = groups.get(id) ?? { id, title, files: [] };
    g.files.push(f);
    groups.set(id, g);
  };
  for (const f of files) {
    if (f.entryKind === "sample") put("sample", "Sample", f);
    else if (f.presetId === "claude-code" || f.collectionId === "claude-code") put("claude-code", "Claude Code", f);
    else if (f.collectionId) put(`c:${f.collectionId}`, humanize(f.collectionId), f);
    else put("other", "Other JSON files", f);
  }
  const rank = (id: string) => (id === "claude-code" ? 0 : id === "sample" ? 1 : id === "other" ? 9 : 5);
  const scopeRank: Record<string, number> = { user: 0, managed: 1, "project-shared": 2, "project-local": 3 };
  return [...groups.values()]
    .sort((a, b) => rank(a.id) - rank(b.id))
    .map((g) => ({ ...g, files: g.id === "claude-code" ? [...g.files].sort((a, b) => (scopeRank[a.discoveredScope ?? ""] ?? 9) - (scopeRank[b.discoveredScope ?? ""] ?? 9)) : g.files }));
}
