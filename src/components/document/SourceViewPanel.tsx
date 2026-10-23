import clsx from "clsx";
import { CircleAlert, ExternalLink, Eye, RefreshCw } from "lucide-react";
import { useMemo, useState } from "react";
import { reloadDoc } from "../../lib/store/doc-store";
import { api } from "../../lib/tauri-api";
import type { Diagnostic } from "../../lib/types/generated/Diagnostic";
import type { SourceView } from "../../lib/types/generated/SourceView";
import { Button } from "../ui/Button";

/** Read-only, line-numbered source for files that cannot be edited (DD4). Secrets stay masked until revealed (R17). */
export function SourceViewPanel({ fileId, view, diagnostics }: { fileId: string; view: SourceView; diagnostics: Diagnostic[] }) {
  const [revealed, setRevealed] = useState<Record<number, string>>({});
  const primary = diagnostics.find((d) => d.severity === "error") ?? diagnostics[0];
  const lines = useMemo(() => {
    let offset = 0;
    return view.text.split("\n").map((text, i) => {
      const start = offset;
      offset += text.length + 1;
      const masks = view.masks.filter((m) => m.start >= start && m.end <= start + text.length).map((m) => ({ ...m, start: m.start - start, end: m.end - start }));
      return { n: i + 1, text, masks };
    });
  }, [view]);
  const highlight = new Set(view.highlightLines);

  const reveal = async (id: number) => {
    const v = await api.revealMaskedValue(fileId, id);
    setRevealed((r) => ({ ...r, [id]: v }));
  };

  return (
    <div data-testid="source-view" className="w-full min-[1600px]:mx-auto min-[1600px]:max-w-[960px]">
      {primary ? (
        <div role="alert" className="m-4 rounded-md border border-danger/60 p-4">
          <div className="flex items-start gap-3">
            <CircleAlert className="mt-0.5 size-5 shrink-0 text-danger" aria-hidden />
            <div className="min-w-0 flex-1">
              <p className="font-medium text-fg">{primary.message}</p>
              <p className="mono mt-1 text-[0.8125rem] text-muted">
                {primary.code}
                {primary.line ? ` · line ${primary.line}${primary.column ? `, column ${primary.column}` : ""}` : ""}
              </p>
              <p className="mt-2 text-sm text-muted">This file can&apos;t be edited as a form until the problem is fixed. Open it in your editor, fix it, then reload.</p>
              <div className="mt-3 flex gap-2">
                <Button onClick={() => void api.openInExternalEditor(fileId)}>
                  <ExternalLink className="size-4" aria-hidden /> Open in editor
                </Button>
                <Button onClick={() => void reloadDoc(fileId)}>
                  <RefreshCw className="size-4" aria-hidden /> Reload
                </Button>
              </div>
            </div>
          </div>
        </div>
      ) : null}
      <pre aria-label="File source, read-only" tabIndex={0} className="mono mx-4 mb-6 overflow-auto rounded-md border border-line text-[0.8125rem] leading-6">
        {lines.map((l) => {
          const isErr = highlight.has(l.n);
          const showCaret = view.caret?.line === l.n;
          return (
            <div key={l.n} data-line={l.n} data-error={isErr || undefined}>
              <div className={clsx("flex", isErr && "bg-danger/15")}>
                <span aria-hidden className="w-12 shrink-0 select-none pr-3 text-right text-muted">{l.n}</span>
                <code className="min-w-0 flex-1 whitespace-pre pr-3">{renderLine(l.text, l.masks, revealed)}</code>
                {l.masks.some((m) => revealed[m.id] === undefined) ? (
                  <button type="button" onClick={() => l.masks.forEach((m) => void reveal(m.id))} className="mr-2 inline-flex items-center gap-1 self-center rounded border border-ctl px-1.5 text-xs text-fg hover:bg-hover" aria-label={`Reveal masked value on line ${l.n}`}>
                    <Eye className="size-3" aria-hidden /> Reveal
                  </button>
                ) : null}
              </div>
              {showCaret ? (
                <div aria-hidden className="flex bg-danger/15 text-danger">
                  <span className="w-12 shrink-0" />
                  <span style={{ paddingLeft: `${Math.max(0, (view.caret?.column ?? 1) - 1)}ch` }}>▲</span>
                </div>
              ) : null}
            </div>
          );
        })}
      </pre>
    </div>
  );
}

function renderLine(text: string, masks: { id: number; start: number; end: number }[], revealed: Record<number, string>) {
  if (!masks.length) return text;
  const out: React.ReactNode[] = [];
  let at = 0;
  for (const m of masks.sort((a, b) => a.start - b.start)) {
    out.push(text.slice(at, m.start));
    out.push(<span key={m.id} data-masked={revealed[m.id] === undefined}>{revealed[m.id] ?? text.slice(m.start, m.end)}</span>);
    at = m.end;
  }
  out.push(text.slice(at));
  return out;
}
