// Keyboard map (DD14). `Mod` is Ctrl, or Cmd on macOS. Kept pure so it can be tested.

export const FIELD_SEARCH_ID = "field-search";

export type ShortcutAction =
  | "command-search" | "field-search" | "save" | "undo" | "redo" | "toggle-sidebar" | "back" | "cycle-landmark" | "cycle-landmark-back";

export const isMac = () => typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent);

export const isTextTarget = (t: EventTarget | null) => {
  const el = t as HTMLElement | null;
  if (!el || !el.tagName) return false;
  if (el.isContentEditable) return true;
  if (el.tagName === "TEXTAREA" || el.tagName === "SELECT") return true;
  if (el.tagName === "INPUT") {
    const type = (el as HTMLInputElement).type;
    return !["checkbox", "radio", "button", "submit"].includes(type);
  }
  return false;
};

type KeyLike = Pick<KeyboardEvent, "key" | "ctrlKey" | "metaKey" | "shiftKey" | "altKey"> & { target?: EventTarget | null };

export function matchShortcut(e: KeyLike, mac = isMac()): ShortcutAction | null {
  const mod = mac ? e.metaKey : e.ctrlKey;
  const k = e.key.toLowerCase();
  if (e.key === "F6") return e.shiftKey ? "cycle-landmark-back" : "cycle-landmark";
  if (e.altKey && !mod && e.key === "ArrowLeft") return "back";
  if (!mod || e.altKey) return null;
  if (k === "k" && !e.shiftKey) return "command-search";
  if (k === "f" && !e.shiftKey) return "field-search";
  if (k === "s" && !e.shiftKey) return "save";
  if (k === "b" && !e.shiftKey) return "toggle-sidebar";
  // Undo/redo only when focus is not in a text field, so native text undo keeps working.
  if (!isTextTarget(e.target ?? null)) {
    if (k === "z" && !e.shiftKey) return "undo";
    if ((k === "z" && e.shiftKey) || (k === "y" && !e.shiftKey)) return "redo";
  }
  return null;
}

export const SHORTCUT_LIST: { keys: string; what: string }[] = [
  { keys: "Mod+K", what: "Go to a setting in this file" },
  { keys: "Mod+F", what: "Focus the form filter" },
  { keys: "Mod+S", what: "Save changes" },
  { keys: "Mod+Z", what: "Undo (when not typing in a field)" },
  { keys: "Mod+Shift+Z", what: "Redo (when not typing in a field)" },
  { keys: "Mod+B", what: "Show or hide the sidebar" },
  { keys: "Alt+Left", what: "Back from a drill-in view" },
  { keys: "Alt+Up / Alt+Down", what: "Move a list item" },
  { keys: "F6 / Shift+F6", what: "Move between header, sidebar, form and save bar" },
  { keys: "Shift+F10", what: "Open the context menu" },
  { keys: "Esc", what: "Close a dialog or leave a drill-in view" },
];

export const formatKeys = (keys: string, mac = isMac()) => keys.replace(/Mod/g, mac ? "Cmd" : "Ctrl");
