import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { FieldHint } from "../types/generated/FieldHint";
import type { Api, Events } from "./types";

const call = <T>(cmd: string, args?: Record<string, unknown>) => invoke<T>(cmd, args);

export const realApi: Api = {
  listManagedFiles: () => call("list_managed_files"),
  getFileStatuses: () => call("get_file_statuses"),
  getStartupNotices: () => call("get_startup_notices"),
  pickAndAddManagedFile: (displayName) => call("pick_and_add_managed_file", { displayName }),
  pickAndCreateManagedFile: () => call("pick_and_create_managed_file"),
  createSampleFile: () => call("create_sample_file"),
  removeManagedFile: (fileId) => call("remove_managed_file", { fileId }),
  updateManagedFileMetadata: (fileId, patch) => call("update_managed_file_metadata", { fileId, patch }),
  relocateManagedFile: (fileId) => call("relocate_managed_file", { fileId }),
  confirmLinkWrite: (fileId) => call("confirm_link_write", { fileId }),
  listDiscoverableClaudeFiles: () => call("list_discoverable_claude_files"),
  pickProjectDirectory: () => call("pick_project_directory"),
  addDiscoveredFile: (candidateId) => call("add_discovered_file", { candidateId }),
  createMissingFile: (fileId) => call("create_missing_file", { fileId }),
  openDocument: (fileId) => call("open_document", { fileId }),
  reloadDocument: (fileId) => call("reload_document", { fileId }),
  getSubtree: (fileId, pointer, depth) => call("get_subtree", { fileId, pointer, depth }),
  // A schema boolean has no enum; the field engine keys booleans off ["true", "false"].
  getFieldHints: async (fileId, pointers) =>
    (await call<FieldHint[]>("get_field_hints", { fileId, pointers })).map((h) =>
      h.valueType === "boolean" && !h.enumValues ? { ...h, enumValues: ["true", "false"] } : h,
    ),
  getPresetInfo: (fileId) => call("get_preset_info", { fileId }),
  applyDraftOperation: (fileId, op) => call("apply_draft_operation", { fileId, op }),
  removeDraftItems: (fileId, pointers) => call("remove_draft_items", { fileId, pointers }),
  undoDraftOperation: (fileId) => call("undo_draft_operation", { fileId }),
  redoDraftOperation: (fileId) => call("redo_draft_operation", { fileId }),
  revertChange: (fileId, opIndex) => call("revert_change", { fileId, opIndex }),
  discardDraft: (fileId) => call("discard_draft", { fileId }),
  previewDraft: (fileId) => call("preview_draft", { fileId }),
  saveDraft: (fileId, expectedFingerprint, acceptWarnings) => call("save_draft", { fileId, expectedFingerprint, acceptWarnings }),
  getConflict: (fileId) => call("get_conflict", { fileId }),
  reapplyDraft: (fileId) => call("reapply_draft", { fileId }),
  saveDraftAsCopy: (fileId) => call("save_draft_as_copy", { fileId }),
  listBackups: (fileId) => call("list_backups", { fileId }),
  previewRestore: (fileId, backupId) => call("preview_restore", { fileId, backupId }),
  restoreBackup: (fileId, backupId, expectedFingerprint) => call("restore_backup", { fileId, backupId, expectedFingerprint }),
  pickSchema: (fileId) => call("pick_schema", { fileId }),
  clearSchema: (fileId) => call("clear_schema", { fileId }),
  revealManagedFile: (fileId) => call("reveal_managed_file", { fileId }),
  openInExternalEditor: (fileId) => call("open_in_external_editor", { fileId }),
  revealMaskedValue: (fileId, maskId) => call("reveal_masked_value", { fileId, maskId }),
  listLeftoverTempFiles: (fileId) => call("list_leftover_temp_files", { fileId }),
  removeLeftoverTempFiles: (fileId) => call("remove_leftover_temp_files", { fileId }),
  getPreferences: () => call("get_preferences"),
  setPreferences: (prefs) => call("set_preferences", { prefs }),
};

export const realEvents: Events = {
  listen: async (event, handler) => {
    const un = await listen<never>(event, (e) => handler(e.payload as never));
    return un;
  },
};
