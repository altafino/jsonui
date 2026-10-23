import * as RG from "@radix-ui/react-radio-group";
import * as Switch from "@radix-ui/react-switch";
import * as Tabs from "@radix-ui/react-tabs";
import clsx from "clsx";
import type { ReactNode } from "react";
import { SHORTCUT_LIST, formatKeys } from "../../lib/shortcuts";
import { useApp } from "../../lib/store/app-store";
import { api, toCoreError } from "../../lib/tauri-api";
import type { Density } from "../../lib/types/generated/Density";
import type { ThemePref } from "../../lib/types/generated/ThemePref";
import { Button } from "../ui/Button";
import { Dialog } from "../ui/Dialog";
import { Chip } from "../ui/Chip";

function Segmented<T extends string>({ label, value, options, onChange }: { label: string; value: T; options: [T, string][]; onChange: (v: T) => void }) {
  return (
    <RG.Root aria-label={label} value={value} onValueChange={(v) => onChange(v as T)} className="inline-flex overflow-hidden rounded-md border border-ctl">
      {options.map(([v, t], i) => (
        <RG.Item key={v} value={v} className={clsx("h-9 px-4 text-sm hover:bg-hover data-[state=checked]:bg-[color-mix(in_srgb,var(--accent)_30%,transparent)] data-[state=checked]:font-semibold", i > 0 && "border-l border-ctl")}>
          {t}
        </RG.Item>
      ))}
    </RG.Root>
  );
}

function Row({ label, hint, children, id }: { label: string; hint?: string; children: ReactNode; id: string }) {
  return (
    <div className="flex items-center justify-between gap-6 border-b border-line py-3">
      <div className="min-w-0">
        <p id={id}>{label}</p>
        {hint ? <p className="text-sm text-muted">{hint}</p> : null}
      </div>
      <div className="shrink-0">{children}</div>
    </div>
  );
}

const TAB = "-mb-px border-b-2 border-transparent px-3 py-2 text-[0.9375rem] text-muted hover:text-fg data-[state=active]:border-accent data-[state=active]:text-fg";

export function SettingsDialog({ tab = "appearance" }: { tab?: "appearance" | "shortcuts" | "files" }) {
  const close = useApp((s) => s.closeDialog);
  const prefs = useApp((s) => s.prefs);
  const setPrefs = useApp((s) => s.setPrefs);
  const files = useApp((s) => s.files);
  const refresh = useApp((s) => s.refreshFiles);
  const openDialog = useApp((s) => s.openDialog);

  const schema = async (id: string, clear: boolean) => {
    try {
      if (clear) await api.clearSchema(id);
      else await api.pickSchema(id);
      await refresh();
    } catch (e) {
      useApp.getState().announce(toCoreError(e).message, "assertive");
    }
  };

  return (
    <Dialog open onClose={close} title="Settings" footer={<Button onClick={close}>Done</Button>} wide>
      <Tabs.Root defaultValue={tab}>
        <Tabs.List aria-label="Settings sections" className="flex gap-1 border-b border-line">
          <Tabs.Trigger value="appearance" className={TAB}>Appearance</Tabs.Trigger>
          <Tabs.Trigger value="shortcuts" className={TAB}>Shortcuts</Tabs.Trigger>
          <Tabs.Trigger value="files" className={TAB}>Files and presets</Tabs.Trigger>
        </Tabs.List>

        <Tabs.Content value="appearance">
          <Row id="s-theme" label="Theme" hint="System follows your operating system.">
            <Segmented<ThemePref> label="Theme" value={prefs.theme} onChange={(theme) => void setPrefs({ theme })} options={[["system", "System"], ["light", "Light"], ["dark", "Dark"]]} />
          </Row>
          <Row id="s-density" label="Density" hint="Compact fits more rows on screen.">
            <Segmented<Density> label="Density" value={prefs.density} onChange={(density) => void setPrefs({ density })} options={[["comfortable", "Comfortable"], ["compact", "Compact"]]} />
          </Row>
          <Row id="s-font" label="Font size" hint={`${Math.round(prefs.fontScale * 100)}% of the default.`}>
            <input type="range" aria-labelledby="s-font" min={0.875} max={1.5} step={0.125} value={prefs.fontScale} onChange={(e) => void setPrefs({ fontScale: Number(e.target.value) })} className="w-48 accent-[var(--accent)]" />
          </Row>
          <Row id="s-retention" label="Backups to keep per file" hint="Older backups are removed after a successful save. Your files are never deleted.">
            <input type="number" aria-labelledby="s-retention" min={1} max={500} value={prefs.backupRetention} onChange={(e) => void setPrefs({ backupRetention: Math.max(1, Math.min(500, Number(e.target.value) || 1)) })} className="h-9 w-24 rounded-md border border-ctl bg-transparent px-3 mono" />
          </Row>
          <Row id="s-secrets" label="Hide secret values" hint="Token, password and key fields stay masked until you reveal them.">
            <Switch.Root aria-labelledby="s-secrets" checked={prefs.hideSecrets} onCheckedChange={(hideSecrets) => void setPrefs({ hideSecrets })} className="relative h-6 w-11 rounded-full border border-ctl data-[state=checked]:border-accent data-[state=checked]:bg-accent">
              <Switch.Thumb className="block size-4 translate-x-[3px] rounded-full bg-ctl transition-transform data-[state=checked]:translate-x-[22px] data-[state=checked]:bg-white" />
            </Switch.Root>
          </Row>
        </Tabs.Content>

        <Tabs.Content value="shortcuts" className="pt-3">
          <table className="w-full text-left">
            <caption className="sr-only">Keyboard shortcuts</caption>
            <thead className="sr-only"><tr><th>Keys</th><th>Action</th></tr></thead>
            <tbody>
              {SHORTCUT_LIST.map((s) => (
                <tr key={s.keys} className="border-b border-line">
                  <td className="py-2 pr-6"><kbd className="mono rounded border border-line px-1.5 py-0.5 text-[0.8125rem]">{formatKeys(s.keys)}</kbd></td>
                  <td className="py-2">{s.what}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </Tabs.Content>

        <Tabs.Content value="files" className="pt-3">
          {files.length === 0 ? <p className="py-4 text-sm text-muted">No files yet.</p> : null}
          <ul className="divide-y divide-[var(--border)]">
            {files.map((f) => (
              <li key={f.id} className="flex items-center gap-3 py-2.5">
                <div className="min-w-0 flex-1">
                  <p className="truncate">{f.displayName}</p>
                  <p className="mono truncate text-[0.8125rem] text-muted">{f.absolutePath}</p>
                  <p className="mt-0.5 flex flex-wrap gap-1.5">
                    {f.presetId ? <Chip>Preset: {f.presetId}</Chip> : <Chip>No preset</Chip>}
                    {f.localSchemaPath ? <Chip>Schema: {f.localSchemaPath}</Chip> : null}
                  </p>
                </div>
                {f.localSchemaPath ? <Button onClick={() => void schema(f.id, true)} aria-label={`Clear schema for ${f.displayName}`}>Clear schema</Button> : <Button onClick={() => void schema(f.id, false)} aria-label={`Choose schema for ${f.displayName}`}>Choose schema…</Button>}
                <Button variant="danger" onClick={() => openDialog({ kind: "remove", fileId: f.id })} aria-label={`Remove ${f.displayName}`}>Remove</Button>
              </li>
            ))}
          </ul>
        </Tabs.Content>
      </Tabs.Root>
    </Dialog>
  );
}
