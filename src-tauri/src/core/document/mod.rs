//! Document engine (document lane).
//!
//! One parse per document (eng R3): a lossless tree per syntax ([`Syntax`]:
//! the jsonc-parser CST for JSON/JSONC, `yaml-edit` for YAML, `toml_edit` for
//! TOML) is the single source of truth. The UI tree, change lists and the
//! `serde_json::Value` used for schema validation all derive from it through
//! the neutral [`value::Val`]. Rust owns the draft (R4): ops apply in place to
//! the live tree (R13); undo/redo replay the op list from the original bytes.
//! Leading UTF-8 BOM is stripped and restored (R10).

mod changes;
mod cst;
mod lines;
mod ops;
mod review;
mod scan;
mod syntax;
#[cfg(test)]
mod tests;
mod toml;
mod value;
mod yaml;

use std::cell::RefCell;

use crate::core::patch::{parse_pointer, pointers_overlap};
use crate::core::persistence::fingerprint;
use crate::core::security::redact::default_is_secret;
use crate::core::types::{
    ChangeKind, ChangeSummary, ConflictInfo, Diagnostic, DocKind, DocumentSnapshot, DraftOp,
    EditResult, Fingerprint, LineEnding, NodeKind, Pointer, SemanticChange, SourceView, TreeNode,
};
use crate::core::{CoreError, CoreResult, ErrorCode};

use self::ops::OpEffect;
use self::scan::Loaded;
use self::syntax::Syntax;
use self::value::Val;

const BOM: [u8; 3] = [0xEF, 0xBB, 0xBF];
const MASKED_TEXT: &str = "••••";

/// Predicate deciding whether the value at a pointer is secret-like and must
/// be masked in diffs and change lists (spec §6/§9, A30). Supplied by the
/// caller (preset hints + user masking); the document lane adds defaults.
pub type SecretPredicate<'a> = &'a dyn Fn(&str) -> bool;

/// An open document plus its draft.
pub struct DocumentSession {
    file_id: String,
    kind: DocKind,
    /// Exact file bytes the draft is based on (BOM included).
    original: Vec<u8>,
    fingerprint: Fingerprint,
    has_bom: bool,
    line_ending: LineEnding,
    /// The live tree; `None` when the file cannot be edited.
    root: Option<Box<dyn Syntax>>,
    /// Why the file cannot be edited (empty when it can).
    blocked: Vec<Diagnostic>,
    source_view: Option<SourceView>,
    /// Original text per mask id: source-view masks first, then the latest review's.
    masks: RefCell<Vec<String>>,
    source_mask_count: usize,
    ops: Vec<DraftOp>,
    redo: Vec<DraftOp>,
    /// One effect per op, same order.
    effects: Vec<OpEffect>,
    revision: u64,
}

// SAFETY: the jsonc-parser CST and yaml-edit's rowan tree are `Rc`-based,
// hence `!Send`. Every `Rc` reachable from a session is created, cloned and
// dropped inside that session's methods: no tree handle is ever returned or
// stored elsewhere (the public API only deals in owned DTOs and bytes). Moving the whole session to another thread therefore
// moves every reference count with it. `Sync` is deliberately not implemented,
// so callers must serialize access (e.g. `Mutex<DocumentSession>`).
unsafe impl Send for DocumentSession {}

fn invalid(message: impl Into<String>) -> CoreError {
    CoreError::new(ErrorCode::InvalidOperation, message)
}

fn not_editable() -> CoreError {
    CoreError::new(
        ErrorCode::NotEditable,
        "This file cannot be edited in its current state",
    )
}

impl DocumentSession {
    /// Load bytes. Never fails: invalid syntax, duplicate keys, non-UTF-8,
    /// or empty input produce a non-editable session with diagnostics and,
    /// for syntax/duplicate errors, a masked `SourceView` (design DD4, R17).
    pub fn load(file_id: &str, bytes: Vec<u8>, kind: DocKind) -> Self {
        let mut session = Self {
            file_id: file_id.to_string(),
            kind,
            original: Vec::new(),
            fingerprint: String::new(),
            has_bom: false,
            line_ending: LineEnding::None,
            root: None,
            blocked: Vec::new(),
            source_view: None,
            masks: RefCell::new(Vec::new()),
            source_mask_count: 0,
            ops: Vec::new(),
            redo: Vec::new(),
            effects: Vec::new(),
            revision: 0,
        };
        let loaded = scan::load(&bytes, kind);
        session.install(bytes, loaded);
        session
    }

    /// Make `bytes` the original this draft is based on.
    fn install(&mut self, bytes: Vec<u8>, loaded: Loaded) {
        self.fingerprint = fingerprint(&bytes);
        self.original = bytes;
        self.has_bom = loaded.has_bom;
        self.line_ending = loaded.line_ending;
        match loaded.result {
            Ok(root) => {
                self.root = Some(root);
                self.blocked = Vec::new();
                self.source_view = None;
                *self.masks.borrow_mut() = Vec::new();
            }
            Err(blocked) => {
                self.root = None;
                self.blocked = blocked.diagnostics;
                let (view, originals) = blocked.source.unzip();
                self.source_view = view;
                *self.masks.borrow_mut() = originals.unwrap_or_default();
            }
        }
        self.source_mask_count = self.masks.borrow().len();
    }

    fn editable_root(&self) -> CoreResult<&dyn Syntax> {
        self.root.as_deref().ok_or_else(not_editable)
    }

    fn root_mut(&mut self) -> CoreResult<&mut Box<dyn Syntax>> {
        self.root.as_mut().ok_or_else(not_editable)
    }

    fn bom_len(&self) -> usize {
        if self.has_bom { BOM.len() } else { 0 }
    }

    /// The original text without BOM (valid UTF-8 whenever the session is editable).
    fn original_text(&self) -> CoreResult<&str> {
        std::str::from_utf8(&self.original[self.bom_len()..]).map_err(|_| {
            CoreError::new(
                ErrorCode::UnsupportedEncoding,
                "The file is not valid UTF-8",
            )
        })
    }

    fn touched(&self) -> Vec<Pointer> {
        self.effects
            .iter()
            .flat_map(|e| e.touched.iter().cloned())
            .collect()
    }

    /// Tree node for `pointer` with `depth` levels of children.
    fn tree_at(&self, pointer: &str, depth: u32) -> Option<TreeNode> {
        self.root.as_ref()?.tree(pointer, depth, &self.touched())
    }

    /// Snapshot with root children to depth 1.
    pub fn snapshot(&self, read_only: bool) -> DocumentSnapshot {
        DocumentSnapshot {
            file_id: self.file_id.clone(),
            kind: self.kind,
            fingerprint: self.fingerprint.clone(),
            size_bytes: self.original.len() as u64,
            has_bom: self.has_bom,
            line_ending: self.line_ending,
            editable: self.is_editable(),
            read_only,
            root: self.tree_at("", 1),
            source_view: self.source_view.clone(),
            diagnostics: self.diagnostics(),
            revision: self.revision,
            dirty_count: self.ops.len() as u32,
            can_undo: !self.ops.is_empty(),
            can_redo: !self.redo.is_empty(),
        }
    }

    /// Lazy subtree for an expanded branch (R13).
    pub fn subtree(&self, pointer: &str, depth: u32) -> CoreResult<TreeNode> {
        self.editable_root()?;
        parse_pointer(pointer)?;
        self.tree_at(pointer, depth)
            .ok_or_else(|| invalid("The path does not exist in the document").with_path(pointer))
    }

    /// Minimal response for an edit: re-rendered nodes (reshaped containers
    /// and touched nodes, depth 1), vanished pointers, and sync diagnostics.
    ///
    /// Ancestors of a changed node are not re-sent; their `changed` flag is
    /// implied (every ancestor of a changed pointer is changed).
    fn edit_result(&self, touched: &[Pointer], reshaped: &[Pointer]) -> EditResult {
        let mut seen: Vec<&Pointer> = Vec::new();
        let (mut changed, mut removed) = (Vec::new(), Vec::new());
        for pointer in reshaped.iter().chain(touched) {
            if seen.contains(&pointer) {
                continue;
            }
            seen.push(pointer);
            match self.tree_at(pointer, 1) {
                Some(node) => changed.push(node),
                None => removed.push(pointer.clone()),
            }
        }
        EditResult {
            revision: self.revision,
            changed,
            removed,
            reshaped: reshaped.to_vec(),
            dirty_count: self.ops.len() as u32,
            can_undo: !self.ops.is_empty(),
            can_redo: !self.redo.is_empty(),
            diagnostics: self.diagnostics(),
        }
    }

    pub fn apply(&mut self, op: DraftOp) -> CoreResult<EditResult> {
        let effect = self.root_mut()?.apply(&op)?;
        self.redo.clear();
        Ok(self.commit(op, effect))
    }

    fn commit(&mut self, op: DraftOp, effect: OpEffect) -> EditResult {
        let (touched, reshaped) = (effect.touched.clone(), effect.reshaped.clone());
        self.ops.push(op);
        self.effects.push(effect);
        self.revision += 1;
        self.edit_result(&touched, &reshaped)
    }

    /// Re-parse the original and apply `ops` to it; on success this becomes the
    /// live tree. On failure nothing changes.
    fn rebuild(&mut self, ops: Vec<DraftOp>) -> CoreResult<()> {
        let mut root = self.parse_original().ok_or_else(|| {
            CoreError::new(ErrorCode::Internal, "Re-parse of the original failed")
        })?;
        let effects = ops
            .iter()
            .map(|op| root.apply(op))
            .collect::<CoreResult<Vec<_>>>()?;
        self.root = Some(root);
        self.ops = ops;
        self.effects = effects;
        Ok(())
    }

    pub fn undo(&mut self) -> CoreResult<EditResult> {
        self.editable_root()?;
        let mut ops = self.ops.clone();
        let (Some(dropped), Some(effect)) = (ops.pop(), self.effects.last().cloned()) else {
            return Err(invalid("There is nothing to undo"));
        };
        self.rebuild(ops)?;
        self.redo.push(dropped);
        self.revision += 1;
        Ok(self.edit_result(&effect.touched, &effect.reshaped))
    }

    pub fn redo(&mut self) -> CoreResult<EditResult> {
        self.editable_root()?;
        let Some(op) = self.redo.pop() else {
            return Err(invalid("There is nothing to redo"));
        };
        match self.root_mut()?.apply(&op) {
            Ok(effect) => Ok(self.commit(op, effect)),
            Err(e) => {
                self.redo.push(op);
                Err(e)
            }
        }
    }

    /// Does `pointer` address an array in the live document?
    fn is_array_now(&self, pointer: &str) -> bool {
        self.root
            .as_ref()
            .and_then(|r| r.tree(pointer, 0, &[]))
            .is_some_and(|t| t.kind == NodeKind::Array)
    }

    /// First later op that depends on op `index` (DD15, R18), if any.
    fn revert_blocker(&self, index: usize) -> Option<&OpEffect> {
        let is_array = |p: &str| self.is_array_now(p);
        let target = &self.effects[index];
        self.effects[index + 1..].iter().find(|later| {
            target.paths.iter().any(|a| {
                later
                    .paths
                    .iter()
                    .any(|b| pointers_overlap(a, b, &is_array))
            })
        })
    }

    /// Remove one op and replay the rest (design DD15). Errors when a later
    /// op overlaps it (`patch::pointers_overlap`, R18).
    pub fn revert_op(&mut self, op_index: u32) -> CoreResult<EditResult> {
        self.editable_root()?;
        let index = op_index as usize;
        if index >= self.ops.len() {
            return Err(invalid(format!("There is no change number {op_index}")));
        }
        if self.revert_blocker(index).is_some() {
            return Err(invalid(format!(
                "Revert later changes under {} first",
                self.effects[index].pointer
            ))
            .with_path(self.effects[index].pointer.clone()));
        }
        let effect = self.effects[index].clone();
        let mut ops = self.ops.clone();
        ops.remove(index);
        self.rebuild(ops)?;
        self.redo.clear();
        self.revision += 1;
        Ok(self.edit_result(&effect.touched, &effect.reshaped))
    }

    pub fn discard(&mut self) {
        self.redo.clear();
        if self.root.is_some() && self.rebuild(Vec::new()).is_err() {
            // Cannot happen for text that parsed before; fall back to a full reload.
            let bytes = self.original.clone();
            let loaded = scan::load(&bytes, self.kind);
            self.install(bytes, loaded);
        }
        self.ops.clear();
        self.effects.clear();
        self.revision += 1;
    }

    pub fn is_editable(&self) -> bool {
        self.root.is_some()
    }

    pub fn is_dirty(&self) -> bool {
        !self.ops.is_empty()
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn ops(&self) -> &[DraftOp] {
        &self.ops
    }

    pub fn original_bytes(&self) -> &[u8] {
        &self.original
    }

    /// Fingerprint of the original bytes this draft is based on.
    pub fn base_fingerprint(&self) -> &Fingerprint {
        &self.fingerprint
    }

    /// Exact bytes Save would write (BOM restored, untouched bytes identical).
    pub fn candidate_bytes(&self) -> Vec<u8> {
        let Some(root) = &self.root else {
            return self.original.clone();
        };
        if self.ops.is_empty() {
            return self.original.clone();
        }
        let text = root.render();
        let mut out = Vec::with_capacity(self.bom_len() + text.len());
        if self.has_bom {
            out.extend_from_slice(&BOM);
        }
        out.extend_from_slice(text.as_bytes());
        out
    }

    /// Current draft as a serde value for schema validation (None if not editable).
    pub fn to_value(&self) -> Option<serde_json::Value> {
        Some(self.root.as_ref()?.value().to_json())
    }

    /// Synchronous diagnostics (syntax, duplicate keys, type) of the draft.
    ///
    /// An editable draft has none: the ops keep the CST syntactically valid
    /// and reject duplicate keys, so only the load-time blockers are reported.
    pub fn diagnostics(&self) -> Vec<Diagnostic> {
        self.blocked.clone()
    }

    /// Semantic change list of the draft, one line per op (design DD15).
    fn semantic_changes(&self, secret: SecretPredicate) -> Vec<SemanticChange> {
        self.effects
            .iter()
            .enumerate()
            .map(|(i, e)| {
                let masked = matches!(
                    e.kind,
                    ChangeKind::Added | ChangeKind::Removed | ChangeKind::Changed
                ) && secret(&e.pointer);
                let shown = |t: &Option<String>| {
                    t.as_ref().map(|t| {
                        if masked {
                            MASKED_TEXT.to_string()
                        } else {
                            t.clone()
                        }
                    })
                };
                SemanticChange {
                    pointer: e.pointer.clone(),
                    kind: e.kind,
                    old_text: shown(&e.old),
                    new_text: shown(&e.new),
                    masked,
                    op_index: Some(i as u32),
                    revertible: self.revert_blocker(i).is_none(),
                }
            })
            .collect()
    }

    /// Review payload; `warnings`/`errors` left empty for the caller to fill
    /// from schema validation.
    pub fn review(&self, is_secret: SecretPredicate) -> ChangeSummary {
        let secret = |p: &str| is_secret(p) || default_is_secret(p);
        ChangeSummary {
            changes: self.semantic_changes(&secret),
            hunks: self.review_hunks(&secret),
            warnings: Vec::new(),
            errors: Vec::new(),
        }
    }

    /// Conflict payload against new disk bytes (design DD16, R16, C4).
    pub fn conflict(&self, disk_bytes: &[u8], is_secret: SecretPredicate) -> ConflictInfo {
        let secret = |p: &str| is_secret(p) || default_is_secret(p);
        let disk_fingerprint = fingerprint(disk_bytes);
        let draft_changes = self.semantic_changes(&secret);
        let disk_root = match scan::load(disk_bytes, self.kind).result {
            Ok(root) => root,
            Err(blocked) => {
                return ConflictInfo {
                    disk_fingerprint,
                    disk_changes: Vec::new(),
                    draft_changes,
                    overlapping: Vec::new(),
                    can_reapply: false,
                    disk_parse_error: blocked.diagnostics.into_iter().next(),
                };
            }
        };
        let diff = self.disk_diff(disk_root.as_ref());
        let disk_changes = diff
            .changes
            .into_iter()
            .map(|c| {
                let masked = secret(&c.pointer);
                let shown =
                    |t: Option<String>| t.map(|t| if masked { MASKED_TEXT.to_string() } else { t });
                SemanticChange {
                    pointer: c.pointer,
                    kind: c.kind,
                    old_text: shown(c.old),
                    new_text: shown(c.new),
                    masked,
                    op_index: None,
                    revertible: false,
                }
            })
            .collect();
        let can_reapply = self.is_editable() && diff.overlapping.is_empty();
        ConflictInfo {
            disk_fingerprint,
            disk_changes,
            draft_changes,
            overlapping: diff.overlapping,
            can_reapply,
            disk_parse_error: None,
        }
    }

    /// Original → disk changes (C4) and the draft pointers that overlap them (R16).
    fn disk_diff(&self, disk_root: &dyn Syntax) -> DiskDiff {
        let disk = disk_root.value();
        let Some(original) = self.original_value() else {
            return DiskDiff::default();
        };
        let changes = changes::diff_values(&original, &disk);
        let is_array = |p: &str| {
            parse_pointer(p).is_ok_and(|segments| {
                [&original, &disk]
                    .iter()
                    .any(|v| matches!(v.lookup(&segments), Some(Val::Array(_))))
            })
        };
        let mut overlapping: Vec<Pointer> = Vec::new();
        for path in self.effects.iter().flat_map(|e| &e.paths) {
            if !overlapping.contains(path)
                && changes
                    .iter()
                    .any(|c| pointers_overlap(path, &c.pointer, &is_array))
            {
                overlapping.push(path.clone());
            }
        }
        DiskDiff {
            changes,
            overlapping,
        }
    }

    /// A fresh parse of the original bytes (None when not editable).
    fn parse_original(&self) -> Option<Box<dyn Syntax>> {
        self.root.as_ref()?;
        scan::load(&self.original, self.kind).result.ok()
    }

    /// The original document as a neutral value tree (re-parsed; only on conflict/review).
    fn original_value(&self) -> Option<Val> {
        Some(self.parse_original()?.value())
    }

    /// Replay the draft onto new disk bytes and rebase (C3). Errors with
    /// `ExternalChangeConflict` if any op overlaps a disk change.
    pub fn reapply_onto(&mut self, disk_bytes: Vec<u8>) -> CoreResult<EditResult> {
        self.editable_root()?;
        let loaded = scan::load(&disk_bytes, self.kind);
        let mut disk_root = loaded.result.map_err(|b| b.to_error())?;
        let diff = self.disk_diff(disk_root.as_ref());
        if let Some(path) = diff.overlapping.first() {
            return Err(CoreError::new(
                ErrorCode::ExternalChangeConflict,
                "The file changed on disk where your draft also changed",
            )
            .with_path(path.clone())
            .with_remediation("Discard your draft and reload, or save your version as a copy."));
        }
        let effects = self
            .ops
            .iter()
            .map(|op| disk_root.apply(op))
            .collect::<CoreResult<Vec<_>>>()
            .map_err(|e| {
                CoreError::new(
                    ErrorCode::ExternalChangeConflict,
                    "Your changes no longer fit the file on disk",
                )
                .with_path(e.path.unwrap_or_default())
            })?;
        let rebased = Loaded {
            has_bom: loaded.has_bom,
            line_ending: loaded.line_ending,
            result: Ok(disk_root),
        };
        self.install(disk_bytes, rebased);
        self.effects = effects;
        self.redo.clear();
        self.revision += 1;
        Ok(EditResult {
            revision: self.revision,
            changed: self.tree_at("", 1).into_iter().collect(),
            removed: Vec::new(),
            reshaped: vec![String::new()],
            dirty_count: self.ops.len() as u32,
            can_undo: !self.ops.is_empty(),
            can_redo: false,
            diagnostics: self.diagnostics(),
        })
    }

    /// Adopt a just-committed save: candidate becomes the new original (C3-style rebase, empty op list).
    pub fn adopt_saved(&mut self, saved_bytes: Vec<u8>) {
        let loaded = scan::load(&saved_bytes, self.kind);
        self.install(saved_bytes, loaded);
        self.ops.clear();
        self.redo.clear();
        self.effects.clear();
        self.revision += 1;
    }

    /// Original text of a masked span in the current source view or review.
    pub fn reveal_masked(&self, mask_id: u32) -> CoreResult<String> {
        self.masks
            .borrow()
            .get(mask_id as usize)
            .cloned()
            .ok_or_else(|| invalid(format!("There is no masked value {mask_id}")))
    }

    /// Semantic change list of draft vs original (used by the Review dialog).
    pub fn changed_pointers(&self) -> Vec<Pointer> {
        let mut out: Vec<Pointer> = Vec::new();
        for e in &self.effects {
            if !out.contains(&e.pointer) {
                out.push(e.pointer.clone());
            }
        }
        out
    }
}

#[derive(Default)]
struct DiskDiff {
    changes: Vec<changes::ValueChange>,
    overlapping: Vec<Pointer>,
}
