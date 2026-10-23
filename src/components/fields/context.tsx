import { createContext, useContext, useEffect } from "react";
import { humanize, parsePointer } from "../../lib/json/pointer";
import { requestHints, useDocs } from "../../lib/store/doc-store";
import type { CachedNode } from "../../lib/store/cache";
import type { Diagnostic } from "../../lib/types/generated/Diagnostic";
import type { FieldHint } from "../../lib/types/generated/FieldHint";
import type { SemanticChange } from "../../lib/types/generated/SemanticChange";

export interface FieldEnv {
  fileId: string;
  /** Read-only, not editable or frozen by an outside-change conflict. */
  frozen: boolean;
  hideSecrets: boolean;
  showAdvanced: boolean;
  /** Search or Changed-only is active: containers on the match path open and non-matches hide. */
  filterActive: boolean;
  visible: Set<string> | null;
  itemSummaryOverrides?: Record<string, string>;
}

export const FieldEnvContext = createContext<FieldEnv | null>(null);

export function useEnv(): FieldEnv {
  const e = useContext(FieldEnvContext);
  if (!e) throw new Error("FieldEnvContext missing");
  return e;
}

export function useNode(pointer: string): CachedNode | undefined {
  const { fileId } = useEnv();
  return useDocs((s) => s.docs[fileId]?.nodes[pointer]);
}

/** Fetches the hint lazily; returns undefined until it arrives. */
export function useHint(pointer: string): FieldHint | undefined {
  const { fileId } = useEnv();
  const hint = useDocs((s) => s.docs[fileId]?.hints[pointer]);
  useEffect(() => {
    if (!hint) requestHints(fileId, [pointer]);
  }, [fileId, pointer, hint]);
  return hint;
}

const EMPTY: SemanticChange[] = [];
export function useChanges(): SemanticChange[] {
  const { fileId } = useEnv();
  return useDocs((s) => s.docs[fileId]?.changes) ?? EMPTY;
}

export function useChangeFor(pointer: string): SemanticChange | undefined {
  return useChanges().find((c) => c.pointer === pointer);
}

const NO_DIAGS: Diagnostic[] = [];
export function useDiagnosticsFor(pointer: string): Diagnostic[] {
  const { fileId } = useEnv();
  const sync = useDocs((s) => s.docs[fileId]?.syncDiagnostics) ?? NO_DIAGS;
  const schema = useDocs((s) => s.docs[fileId]?.schemaDiagnostics) ?? NO_DIAGS;
  return [...sync, ...schema].filter((d) => d.pointer === pointer);
}

export const lastToken = (pointer: string) => parsePointer(pointer).slice(-1)[0] ?? "";
export const idFor = (pointer: string, part: string) => `f-${part}-${pointer.replace(/[^A-Za-z0-9]/g, "_")}`;
export const jsonString = (s: string) => JSON.stringify(s);

const norm = (s: string) => s.toLowerCase().replace(/[^a-z0-9]/g, "");
/** DD1: the raw key shows only when the label (schema title) says something else, or with Show advanced. */
export const shouldShowKey = (key: string | null, title: string | null | undefined, advanced: boolean) =>
  key !== null && (advanced || (!!title && norm(title) !== norm(humanize(key)) && title !== key));
