import clsx from "clsx";
import { Eye } from "lucide-react";
import { useState } from "react";
import { api } from "../../lib/tauri-api";
import type { DiffHunk } from "../../lib/types/generated/DiffHunk";
import type { DiffLine } from "../../lib/types/generated/DiffLine";

const SIGN = { added: "+", removed: "−", context: " " } as const;

/** Changed hunks with context. Sign column plus tint, so change is not color alone. Masked values reveal on demand. */
export function TextDiff({ fileId, hunks }: { fileId: string; hunks: DiffHunk[] }) {
  const [revealed, setRevealed] = useState<Record<number, string>>({});
  if (hunks.length === 0) return <p className="py-6 text-sm text-muted">No text changes.</p>;

  const reveal = async (id: number) => {
    try {
      const v = await api.revealMaskedValue(fileId, id);
      setRevealed((r) => ({ ...r, [id]: v }));
    } catch {
      /* mask expired; leave masked */
    }
  };

  const render = (l: DiffLine) => {
    if (!l.masks.length) return l.text;
    const out: React.ReactNode[] = [];
    let at = 0;
    for (const m of [...l.masks].sort((a, b) => a.start - b.start)) {
      out.push(l.text.slice(at, m.start));
      out.push(<span key={m.id} data-masked={revealed[m.id] === undefined}>{revealed[m.id] ?? l.text.slice(m.start, m.end)}</span>);
      at = m.end;
    }
    out.push(l.text.slice(at));
    return out;
  };

  return (
    <div className="space-y-3" data-testid="text-diff">
      {hunks.map((h, i) => (
        <pre key={i} className="mono overflow-auto rounded-md border border-line text-[0.8125rem] leading-6" aria-label={`Change ${i + 1}`}>
          {h.lines.map((l, j) => (
            <div
              key={j}
              data-kind={l.kind}
              className={clsx("flex", l.kind === "added" && "bg-ok/15", l.kind === "removed" && "bg-danger/15")}
            >
              <span aria-hidden className="w-10 shrink-0 select-none px-1 text-right text-muted">{l.oldLine ?? ""}</span>
              <span aria-hidden className="w-10 shrink-0 select-none px-1 text-right text-muted">{l.newLine ?? ""}</span>
              <span aria-hidden className={clsx("w-5 shrink-0 select-none text-center", l.kind === "added" && "text-ok", l.kind === "removed" && "text-danger")}>{SIGN[l.kind]}</span>
              <code className="min-w-0 flex-1 whitespace-pre pr-2">
                <span className="sr-only">{l.kind === "added" ? "Added: " : l.kind === "removed" ? "Removed: " : ""}</span>
                {render(l)}
              </code>
              {l.masks.some((m) => revealed[m.id] === undefined) ? (
                <button type="button" aria-label="Reveal masked value" onClick={() => l.masks.forEach((m) => void reveal(m.id))} className="mr-2 inline-flex items-center gap-1 self-center rounded border border-ctl px-1.5 text-xs hover:bg-hover">
                  <Eye className="size-3" aria-hidden /> Reveal
                </button>
              ) : null}
            </div>
          ))}
        </pre>
      ))}
    </div>
  );
}
