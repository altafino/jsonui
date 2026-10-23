// Typed IPC surface: one method per command in docs/contract.md. UI never sends file paths (eng R6).
import type { BackupEntry } from "../types/generated/BackupEntry";
import type { ChangeSummary } from "../types/generated/ChangeSummary";
import type { ConflictInfo } from "../types/generated/ConflictInfo";
import type { CoreError } from "../types/generated/CoreError";
import type { Diagnostic } from "../types/generated/Diagnostic";
import type { DiscoveredCandidate } from "../types/generated/DiscoveredCandidate";
import type { DocumentSnapshot } from "../types/generated/DocumentSnapshot";
import type { DraftOp } from "../types/generated/DraftOp";
import type { EditResult } from "../types/generated/EditResult";
import type { FieldHint } from "../types/generated/FieldHint";
import type { FileStatus } from "../types/generated/FileStatus";
import type { ManagedDocument } from "../types/generated/ManagedDocument";
import type { MetadataPatch } from "../types/generated/MetadataPatch";
import type { Notice } from "../types/generated/Notice";
import type { Preferences } from "../types/generated/Preferences";
import type { PresetInfo } from "../types/generated/PresetInfo";
import type { SaveResult } from "../types/generated/SaveResult";
import type { TreeNode } from "../types/generated/TreeNode";

export interface Api {
  listManagedFiles(): Promise<ManagedDocument[]>;
  getFileStatuses(): Promise<FileStatus[]>;
  getStartupNotices(): Promise<Notice[]>;
  pickAndAddManagedFile(displayName?: string): Promise<ManagedDocument | null>;
  pickAndCreateManagedFile(): Promise<ManagedDocument | null>;
  createSampleFile(): Promise<ManagedDocument>;
  removeManagedFile(fileId: string): Promise<void>;
  updateManagedFileMetadata(fileId: string, patch: MetadataPatch): Promise<ManagedDocument>;
  relocateManagedFile(fileId: string): Promise<ManagedDocument | null>;
  confirmLinkWrite(fileId: string): Promise<ManagedDocument>;
  listDiscoverableClaudeFiles(): Promise<DiscoveredCandidate[]>;
  pickProjectDirectory(): Promise<DiscoveredCandidate[] | null>;
  addDiscoveredFile(candidateId: string): Promise<ManagedDocument>;
  createMissingFile(fileId: string): Promise<void>;
  openDocument(fileId: string): Promise<DocumentSnapshot>;
  reloadDocument(fileId: string): Promise<DocumentSnapshot>;
  getSubtree(fileId: string, pointer: string, depth: number): Promise<TreeNode>;
  getFieldHints(fileId: string, pointers: string[]): Promise<FieldHint[]>;
  getPresetInfo(fileId: string): Promise<PresetInfo | null>;
  applyDraftOperation(fileId: string, op: DraftOp): Promise<EditResult>;
  undoDraftOperation(fileId: string): Promise<EditResult>;
  redoDraftOperation(fileId: string): Promise<EditResult>;
  revertChange(fileId: string, opIndex: number): Promise<EditResult>;
  discardDraft(fileId: string): Promise<DocumentSnapshot>;
  previewDraft(fileId: string): Promise<ChangeSummary>;
  saveDraft(fileId: string, expectedFingerprint: string, acceptWarnings: boolean): Promise<SaveResult>;
  getConflict(fileId: string): Promise<ConflictInfo>;
  reapplyDraft(fileId: string): Promise<EditResult>;
  saveDraftAsCopy(fileId: string): Promise<ManagedDocument | null>;
  listBackups(fileId: string): Promise<BackupEntry[]>;
  previewRestore(fileId: string, backupId: string): Promise<ChangeSummary>;
  restoreBackup(fileId: string, backupId: string, expectedFingerprint: string): Promise<SaveResult>;
  pickSchema(fileId: string): Promise<ManagedDocument | null>;
  clearSchema(fileId: string): Promise<ManagedDocument>;
  revealManagedFile(fileId: string): Promise<void>;
  openInExternalEditor(fileId: string): Promise<void>;
  revealMaskedValue(fileId: string, maskId: number): Promise<string>;
  listLeftoverTempFiles(fileId: string): Promise<string[]>;
  removeLeftoverTempFiles(fileId: string): Promise<number>;
  getPreferences(): Promise<Preferences>;
  setPreferences(prefs: Preferences): Promise<void>;
}

export type ApiMethod = keyof Api;

export interface EventMap {
  "managed-file-external-change": { fileId: string; fingerprint: string; hasUnsavedChanges: boolean };
  "managed-file-missing": { fileId: string };
  "managed-file-access-changed": { fileId: string; readable: boolean; writable: boolean };
  "draft-diagnostics": { fileId: string; revision: bigint | number; diagnostics: Diagnostic[] };
}

export type Unlisten = () => void;

export interface Events {
  listen<K extends keyof EventMap>(event: K, handler: (payload: EventMap[K]) => void): Promise<Unlisten>;
}

/** Dev-only hooks exposed by the mock backend (never present with the real backend). */
export interface MockControls {
  /** Simulate another program rewriting the file on disk. */
  simulateExternalChange(fileId: string, mutate?: "non-overlapping" | "overlapping" | "invalid"): void;
  simulateMissing(fileId: string): void;
  simulateSaveFailure(code: CoreError["code"] | null): void;
  setSaveDelay(ms: number): void;
  /** Registry id of a file by display name (tests only). */
  idOf(displayName: string): string | undefined;
  reset(): void;
}

export function isCoreError(e: unknown): e is CoreError {
  return typeof e === "object" && e !== null && "code" in e && "message" in e;
}

/** Normalises any rejection into a CoreError for display. */
export function toCoreError(e: unknown): CoreError {
  if (isCoreError(e)) return e;
  return { code: "INTERNAL", message: e instanceof Error ? e.message : String(e), path: null, line: null, column: null, remediation: null };
}
