import { useApp } from "../../lib/store/app-store";
import { ReviewDialog } from "../diff/ReviewDialog";
import { AboutDialog } from "./AboutDialog";
import { AddSettingDialog } from "./AddSettingDialog";
import { CommandSearchDialog } from "./CommandSearchDialog";
import { ConflictDialog } from "./ConflictDialog";
import { DiscoverDialog } from "./DiscoverDialog";
import { HistoryDialog } from "./HistoryDialog";
import { SettingsDialog } from "./SettingsDialog";
import { LinkDialog, ReloadDialog, RemoveDialog, RenameDialog, UnsavedDialog } from "./SimpleDialogs";

/** One modal at a time. Esc and the close button route through closeDialog. */
export function DialogHost() {
  const d = useApp((s) => s.dialog);
  if (!d) return null;
  switch (d.kind) {
    case "settings": return <SettingsDialog tab={d.tab} />;
    case "about": return <AboutDialog />;
    case "history": return <HistoryDialog fileId={d.fileId} />;
    case "review": return <ReviewDialog fileId={d.fileId} />;
    case "conflict": return <ConflictDialog fileId={d.fileId} />;
    case "discover": return <DiscoverDialog />;
    case "unsaved": return <UnsavedDialog fromId={d.fromId} />;
    case "link": return <LinkDialog fileId={d.fileId} code={d.code} />;
    case "rename": return <RenameDialog fileId={d.fileId} />;
    case "remove": return <RemoveDialog fileId={d.fileId} />;
    case "reload": return <ReloadDialog fileId={d.fileId} />;
    case "command": return <CommandSearchDialog fileId={useApp.getState().activeFileId ?? ""} />;
    case "addSetting": return <AddSettingDialog fileId={d.fileId} parent={d.parent} name={d.name} />;
    default: {
      const unreachable: never = d;
      return unreachable;
    }
  }
}
