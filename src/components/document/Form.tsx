import { ChevronRight, ArrowLeft } from "lucide-react";
import { useEffect, useMemo } from "react";
import { displayPath, humanize, parentPointer, parsePointer, buildPointer } from "../../lib/json/pointer";
import { useApp } from "../../lib/store/app-store";
import { computeVisible } from "../../lib/store/visibility";
import { drillInto, ensureFullyLoaded, ensureLoaded, patch, setChangedOnly, setQuery, useDocs } from "../../lib/store/doc-store";
import { ArrayList } from "../fields/ArrayList";
import { AddSettingMenu, Children } from "../fields/Containers";
import { FieldEnvContext, useHint, useNode, type FieldEnv } from "../fields/context";
import { Field } from "../fields/Field";
import { Button } from "../ui/Button";
import { EmptyState } from "./EmptyStates";

export function Breadcrumb({ fileId, drillRoot }: { fileId: string; drillRoot: string }) {
  const toks = parsePointer(drillRoot);
  const nodes = useDocs((s) => s.docs[fileId]?.nodes);
  const back = () => drillInto(fileId, parentPointer(drillRoot));
  return (
    <nav aria-label="Breadcrumb" className="flex items-center gap-1 border-b border-line px-4 py-2 text-sm">
      <Button variant="ghost" onClick={back} aria-label="Back" className="h-7 px-2">
        <ArrowLeft className="size-4" aria-hidden />
      </Button>
      <ol className="flex min-w-0 flex-wrap items-center gap-1 mono">
        <li><button type="button" onClick={() => drillInto(fileId, "")} className="rounded px-1.5 py-0.5 hover:bg-hover">$</button></li>
        {toks.map((t, i) => {
          const ptr = buildPointer(toks.slice(0, i + 1));
          const last = i === toks.length - 1;
          const n = nodes?.[ptr];
          const text = n?.index != null ? String(n.index) : humanizeIfPlain(t);
          return (
            <li key={ptr} className="flex items-center gap-1">
              <ChevronRight className="size-3.5 text-muted" aria-hidden />
              {last ? <span aria-current="page" className="px-1.5 font-semibold">{text}</span> : <button type="button" onClick={() => drillInto(fileId, ptr)} className="rounded px-1.5 py-0.5 hover:bg-hover">{text}</button>}
            </li>
          );
        })}
      </ol>
    </nav>
  );
}
const humanizeIfPlain = (t: string) => t;

export function Form({ fileId }: { fileId: string }) {
  const doc = useDocs((s) => s.docs[fileId]);
  const hideSecrets = useApp((s) => s.prefs.hideSecrets);
  const { nodes, hints, query, changedOnly, drillRoot, showAdvanced, presetInfo } = doc;
  const filterActive = query.trim() !== "" || changedOnly;
  const root = nodes[drillRoot] ? drillRoot : "";

  useEffect(() => {
    if (filterActive) void ensureFullyLoaded(fileId);
  }, [filterActive, fileId, nodes]);

  useEffect(() => {
    if (!nodes[drillRoot] && drillRoot !== "" && nodes[""]?.childPointers) patch(fileId, { drillRoot: "" });
    else void ensureLoaded(fileId, root);
  }, [fileId, drillRoot, root, nodes]);

  // Focus requests (search deep links, list reordering).
  useEffect(() => {
    const p = doc.focusPointer;
    if (p === null) return;
    const rowEl = document.getElementById(`row-${p}`);
    const el =
      document.querySelector<HTMLElement>(`[data-handle="${CSS.escape(p)}"]`) ??
      ["[role=radio][aria-checked=true]", "[role=switch]", "input", "select", "textarea", "[role=radio]", "button"].map((q) => rowEl?.querySelector<HTMLElement>(q) ?? null).find(Boolean) ??
      null;
    if (el) {
      el.focus({ preventScroll: false });
      el.scrollIntoView({ block: "nearest" });
      patch(fileId, { focusPointer: null });
    }
  }, [doc.focusPointer, nodes, fileId]);

  const visible = useMemo(() => (filterActive ? computeVisible(nodes, root, { query, changedOnly }, hints) : null), [filterActive, nodes, root, query, changedOnly, hints]);

  const env: FieldEnv = {
    fileId,
    frozen: doc.readOnly || doc.conflict || !doc.editable,
    hideSecrets,
    showAdvanced,
    filterActive,
    visible,
  };

  const node = nodes[root];
  if (!node) return <div className="p-6 text-sm text-muted" aria-busy="true" />;

  let body;
  if (visible && visible.size === 0 && (node.childPointers?.length ?? 0) > 0) {
    body = changedOnly && query.trim() === "" ? (
      <EmptyState title="No unsaved changes" action={<Button onClick={() => setChangedOnly(fileId, false)}>Show all fields</Button>} />
    ) : (
      <EmptyState title={`No fields match "${query.trim()}"`} action={<Button onClick={() => (setQuery(fileId, ""), setChangedOnly(fileId, false))}>Clear search</Button>} />
    );
  } else if (node.kind === "object") {
    body = node.childCount === 0 ? (
      <EmptyState
        title="This file is an empty object."
        action={<AddSettingMenu parent={root} label={root === "" ? "this file" : humanize(parsePointer(root).slice(-1)[0] ?? "")} />}
      />
    ) : (
      <Groups root={root} filterActive={filterActive} presetGroups={root === "" ? presetInfo?.groups : undefined} />
    );
  } else if (node.kind === "array") {
    body = <ArrayList pointer={root} level={1} />;
  } else {
    body = <Field pointer={root} level={1} />;
  }

  return (
    <FieldEnvContext.Provider value={env}>
      {root !== "" ? <Breadcrumb fileId={fileId} drillRoot={root} /> : null}
      <div role="region" aria-label={`Form for ${displayPath(root)}`} className="w-full min-[1600px]:mx-auto min-[1600px]:max-w-[960px]">
        {body}
      </div>
    </FieldEnvContext.Provider>
  );
}

function Groups({ root, filterActive, presetGroups }: { root: string; filterActive: boolean; presetGroups?: { id: string; title: string; pointers: string[] }[] }) {
  const node = useNode(root);
  const hint = useHint(root);
  if (!presetGroups || filterActive) return <Children parent={root} level={1} />;
  const present = new Set(node?.childPointers ?? []);
  const grouped = new Set(presetGroups.flatMap((g) => g.pointers));
  const hasOther = [...present].some((p) => !grouped.has(p)) || (hint?.addableProperties.length ?? 0) > 0;
  return (
    <>
      {presetGroups
        .filter((g) => g.pointers.some((p) => present.has(p)))
        .map((g) => (
          <GroupSection key={g.id} title={g.title}>
            <Children parent={root} level={1} only={(p) => g.pointers.includes(p)} absent={false} />
          </GroupSection>
        ))}
      {hasOther ? (
        <GroupSection title="Other">
          <Children parent={root} level={1} only={(p) => !grouped.has(p)} />
        </GroupSection>
      ) : null}
    </>
  );
}

/** Caps section label (12/500). */
function GroupSection({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section aria-label={title}>
      <h3 className="px-4 pt-4 pb-1 text-xs font-medium uppercase tracking-wider text-muted">{title}</h3>
      {children}
    </section>
  );
}
