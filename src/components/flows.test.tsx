import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { useApp } from "../lib/store/app-store";
import { receiveDiagnostics, useDocs } from "../lib/store/doc-store";
import { bootApp, sleep, type Mock } from "../test/harness";

const doc = () => useDocs.getState().docs[useApp.getState().activeFileId!];
const activeId = () => useApp.getState().activeFileId!;
const status = () => screen.getByTestId("save-status");
const toggleDebug = async (user: ReturnType<typeof userEvent.setup>) => {
  await user.click(await screen.findByRole("switch", { name: "Debug" }));
  await waitFor(() => expect(doc().dirtyCount).toBeGreaterThan(0));
};
const setField = async (user: ReturnType<typeof userEvent.setup>, name: string, text: string) => {
  const input = await screen.findByRole("textbox", { name });
  await user.clear(input);
  if (text) await user.type(input, text);
  await user.tab();
};

describe("empty states (DD5)", () => {
  it("welcome shows the exact copy and three actions", async () => {
    await bootApp({ flags: [] });
    expect(await screen.findByText("Edit JSON settings as forms. Every save makes a backup first.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Add JSON file/ })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Discover Claude Code settings/ })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Try with a sample/ })).toBeInTheDocument();
  });

  it("empty object and history", async () => {
    await bootApp({ open: "empty.json" });
    expect(await screen.findByText("This file is an empty object.")).toBeInTheDocument();
    await userEvent.setup().click(screen.getByRole("button", { name: "History" }));
    expect(await screen.findByText("No backups yet. One is created every time you save.")).toBeInTheDocument();
  });

  it("missing file", async () => {
    const { m } = await bootApp({ flags: ["seeded", "missing"] });
    const gone = useApp.getState().files.find((f) => f.displayName === "gone.json")!;
    await act(async () => {
      await (await import("../lib/store/actions")).activate(gone.id);
    });
    expect(await screen.findByText(/This file no longer exists at/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Locate file" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Remove from ConfigDeck" })).toBeInTheDocument();
    void m;
  });

  it("leftover temp file notice offers removal", async () => {
    const user = userEvent.setup();
    await bootApp({ flags: ["seeded", "leftover"], open: "User settings" });
    expect(await screen.findByTestId("leftover-banner")).toHaveTextContent(".settings.json.configdeck-tmp");
    await user.click(screen.getByRole("button", { name: /Remove leftover/ }));
    await waitFor(() => expect(screen.queryByTestId("leftover-banner")).not.toBeInTheDocument());
  });

  it("registry recovery notice", async () => {
    await bootApp({ flags: ["seeded", "recovered"] });
    expect(await screen.findByTestId("startup-notice")).toHaveTextContent(/could not be read/);
  });
});

describe("save bar (DD2, DD6)", () => {
  it("is footer-only, disabled with a visible reason, and saves with undo", async () => {
    const user = userEvent.setup();
    await bootApp({ open: "feature-flags.json" });
    const save = screen.getByRole("button", { name: "Save changes" });
    expect(save).toBeDisabled();
    expect(status()).toHaveTextContent("No unsaved changes");
    // header carries no save/review buttons
    expect(within(screen.getByRole("banner")).queryByRole("button", { name: /Save|Review/ })).toBeNull();
    expect(within(await screen.findByLabelText("File header")).queryByRole("button", { name: /Save|Review/ })).toBeNull();

    await toggleDebug(user);
    await waitFor(() => expect(status()).toHaveTextContent("Ready · 1 pending change"));
    await user.click(save);
    await waitFor(() => expect(status()).toHaveTextContent(/Saved \d\d:\d\d · backup created/));
    expect(within(status()).getByRole("button", { name: "Undo save" })).toBeInTheDocument();
    expect(doc().dirtyCount).toBe(0);

    await user.click(within(status()).getByRole("button", { name: "Undo save" }));
    await waitFor(() => expect(status()).toHaveTextContent(/Restored/));
    await waitFor(() => expect(screen.getByRole("switch", { name: "Debug" })).not.toBeChecked());
  });

  it("shows the spinner only after 150 ms", async () => {
    const user = userEvent.setup();
    await bootApp({ open: "feature-flags.json", saveDelayMs: 400 });
    await toggleDebug(user);
    await user.click(screen.getByRole("button", { name: "Save changes" }));
    await sleep(40);
    expect(doc().save).toMatchObject({ phase: "saving", spinner: false });
    await sleep(200);
    expect(doc().save).toMatchObject({ phase: "saving", spinner: true });
    await waitFor(() => expect(doc().save.phase).toBe("saved"), { timeout: 2000 });
  });

  it("shows a persistent error strip and keeps the draft when saving fails", async () => {
    const user = userEvent.setup();
    const { m } = await bootApp({ open: "feature-flags.json" });
    await toggleDebug(user);
    m.controls.simulateSaveFailure("BACKUP_FAILED");
    await user.click(screen.getByRole("button", { name: "Save changes" }));
    const strip = await screen.findByTestId("save-error");
    expect(strip).toHaveTextContent("Backup folder is not writable.");
    expect(strip).toHaveTextContent("Choose another backup folder in Settings.");
    expect(strip).toHaveTextContent("BACKUP_FAILED");
    expect(doc().dirtyCount).toBe(1);
    expect(within(strip).getByText(/Couldn.t save/)).toBeInTheDocument();
  });

  it("blocks save on errors with the reason in the bar", async () => {
    const user = userEvent.setup();
    await bootApp({ open: "feature-flags.json" });
    await setField(user, "Application", "");
    await waitFor(() => expect(status()).toHaveTextContent("Fix 1 error to save"), { timeout: 2000 });
    expect(screen.getByRole("button", { name: "Save changes" })).toBeDisabled();
  });

  it("Ctrl+S saves the draft, even while a typed value is still being edited", async () => {
    const user = userEvent.setup();
    await bootApp({ open: "feature-flags.json" });
    const input = await screen.findByRole("textbox", { name: "Display name" });
    await user.clear(input);
    await user.type(input, "Renamed");
    fireEvent.keyDown(input, { key: "s", ctrlKey: true });
    await waitFor(() => expect(status()).toHaveTextContent(/Saved/), { timeout: 2000 });
    expect(doc().nodes["/displayName"].text).toBe("Renamed");
  });

  it("Ctrl+Z undoes outside text fields only", async () => {
    const user = userEvent.setup();
    await bootApp({ open: "feature-flags.json" });
    await toggleDebug(user);
    const input = await screen.findByRole("textbox", { name: "Display name" });
    input.focus();
    fireEvent.keyDown(input, { key: "z", ctrlKey: true });
    await sleep(30);
    expect(doc().dirtyCount).toBe(1);
    (document.activeElement as HTMLElement).blur();
    fireEvent.keyDown(document.body, { key: "z", ctrlKey: true });
    await waitFor(() => expect(doc().dirtyCount).toBe(0));
    fireEvent.keyDown(document.body, { key: "z", ctrlKey: true, shiftKey: true });
    await waitFor(() => expect(doc().dirtyCount).toBe(1));
  });
});

describe("review changes (DD15)", () => {
  it("lists changes with per-change Revert and a text diff tab", async () => {
    const user = userEvent.setup();
    await bootApp({ open: "feature-flags.json" });
    await toggleDebug(user);
    await user.click(screen.getByRole("button", { name: "Review changes" }));
    const dialog = await screen.findByRole("dialog");
    const rows = await within(dialog).findAllByTestId("change-row");
    expect(rows).toHaveLength(1);
    expect(rows[0]).toHaveTextContent("Debug");
    expect(rows[0]).toHaveTextContent("false");
    expect(rows[0]).toHaveTextContent("true");
    await user.click(within(dialog).getByRole("tab", { name: "Text diff" }));
    const diff = await within(dialog).findByTestId("text-diff");
    expect(diff.querySelectorAll('[data-kind="added"]')).toHaveLength(1);
    expect(diff.querySelectorAll('[data-kind="removed"]')).toHaveLength(1);
    await user.click(within(dialog).getByRole("tab", { name: /Changes/ }));
    await user.click(within(dialog).getByRole("button", { name: /Revert Debug/ }));
    await waitFor(() => expect(within(dialog).getByText("No unsaved changes")).toBeInTheDocument());
    expect(doc().dirtyCount).toBe(0);
    expect(within(dialog).getByRole("button", { name: "Save changes" })).toBeDisabled();
  });

  it("masks secret values in the text diff until revealed", async () => {
    const user = userEvent.setup();
    await bootApp({ open: "secrets.json" });
    const token = await screen.findByLabelText("API TOKEN", { selector: "input" });
    await user.clear(token);
    await user.type(token, "sk-new-1111");
    await user.tab();
    await user.click(screen.getByRole("button", { name: "Review changes" }));
    const dialog = await screen.findByRole("dialog");
    await user.click(await within(dialog).findByRole("tab", { name: "Text diff" }));
    const diff = await within(dialog).findByTestId("text-diff");
    expect(diff).not.toHaveTextContent("sk-new-1111");
    expect(diff).not.toHaveTextContent("sk-test-0000000000000000");
    await user.click(within(diff).getAllByRole("button", { name: "Reveal masked value" })[0]);
    await waitFor(() => expect(diff.textContent).toMatch(/sk-(new|test)/));
  });

  it("preset warnings become Save with N warnings after confirmation", async () => {
    const user = userEvent.setup();
    await bootApp({ open: "my-app · Shared" });
    await user.selectOptions(await screen.findByRole("combobox", { name: "Default permission mode" }), '"bypassPermissions"');
    await waitFor(() => expect(doc().schemaDiagnostics.length).toBe(1), { timeout: 2000 });
    await user.click(screen.getByRole("button", { name: "Save changes" }));
    const dialog = await screen.findByRole("dialog");
    expect(await within(dialog).findByTestId("warn-block")).toHaveTextContent("bypassPermissions skips all permission prompts");
    await user.click(within(dialog).getByRole("button", { name: "Save with 1 warning" }));
    await waitFor(() => expect(status()).toHaveTextContent(/Saved/), { timeout: 2000 });
    expect(doc().dirtyCount).toBe(0);
  });
});

describe("conflict (DD16, R16)", () => {
  const conflict = async (mode: "non-overlapping" | "overlapping" | "invalid", m: Mock) => {
    const user = userEvent.setup();
    await toggleDebug(user);
    act(() => m.controls.simulateExternalChange(activeId(), mode));
    await screen.findByTestId("conflict-banner");
    await user.click(within(screen.getByTestId("conflict-banner")).getByRole("button", { name: "Compare" }));
    return { user, dialog: await screen.findByRole("dialog") };
  };

  it("freezes saving and lets a non-overlapping draft be reapplied without writing", async () => {
    const { m } = await bootApp({ open: "feature-flags.json" });
    const { user, dialog } = await conflict("non-overlapping", m);
    expect(screen.getByRole("button", { name: "Save changes", hidden: true })).toBeDisabled();
    expect(await within(dialog).findByRole("list", { name: "Changed on disk" })).toBeInTheDocument();
    expect(within(dialog).getByRole("list", { name: "Your unsaved changes" })).toHaveTextContent("Debug");
    const reapply = within(dialog).getByRole("button", { name: "Reapply my changes to the new version" });
    expect(reapply).toBeEnabled();
    await user.click(reapply);
    await waitFor(() => expect(screen.queryByTestId("conflict-banner")).not.toBeInTheDocument());
    expect(doc().dirtyCount).toBe(1);
    expect(doc().conflict).toBe(false);
    // nothing was written: saving is the user's next, normal step
    expect(await m.api.listBackups(activeId())).toHaveLength(0);
    await user.click(screen.getByRole("button", { name: "Save changes" }));
    await waitFor(() => expect(status()).toHaveTextContent(/Saved/), { timeout: 2000 });
  });

  it("disables Reapply and marks 'both changed' when the same setting changed on disk", async () => {
    const { m } = await bootApp({ open: "feature-flags.json" });
    const { dialog } = await conflict("overlapping", m);
    expect((await within(dialog).findAllByText("both changed")).length).toBeGreaterThan(0);
    expect(within(dialog).getByRole("button", { name: "Reapply my changes to the new version" })).toBeDisabled();
    expect(within(dialog).getByRole("button", { name: "Discard my draft & reload" })).toBeEnabled();
    expect(within(dialog).getByRole("button", { name: "Save my version as a copy" })).toBeEnabled();
  });

  it("an unparseable disk version cannot be reapplied", async () => {
    const { m } = await bootApp({ open: "feature-flags.json" });
    const { dialog } = await conflict("invalid", m);
    expect((await within(dialog).findAllByText(/not valid JSON/)).length).toBeGreaterThan(0);
    expect(within(dialog).getByRole("button", { name: "Reapply my changes to the new version" })).toBeDisabled();
  });

  it("an outside change without a draft reloads quietly", async () => {
    const { m } = await bootApp({ open: "feature-flags.json" });
    await screen.findByRole("switch", { name: "Debug" });
    act(() => m.controls.simulateExternalChange(activeId(), "non-overlapping"));
    await waitFor(() => expect(doc().externalNote).toMatch(/changed outside ConfigDeck/));
    expect(screen.queryByTestId("conflict-banner")).not.toBeInTheDocument();
  });
});

describe("documents, diagnostics and files", () => {
  it("drops draft-diagnostics for stale revisions (R14)", async () => {
    const user = userEvent.setup();
    await bootApp({ open: "feature-flags.json" });
    await toggleDebug(user);
    const rev = doc().revision;
    const warn = { severity: "warning", source: "presetSchema", code: "X", message: "old", pointer: null, line: null, column: null } as const;
    expect(receiveDiagnostics(activeId(), rev - 1, [warn])).toBe(false);
    expect(doc().schemaDiagnostics).toEqual([]);
    expect(receiveDiagnostics(activeId(), BigInt(rev), [warn])).toBe(true);
    expect(doc().schemaDiagnostics).toHaveLength(1);
    // rapid edits: a late result for the previous revision never replaces newer diagnostics
    await user.click(screen.getByRole("switch", { name: "Debug" }));
    await waitFor(() => expect(doc().revision).toBeGreaterThan(rev));
    expect(receiveDiagnostics(activeId(), rev, [{ ...warn, message: "stale" }])).toBe(false);
    expect(doc().schemaDiagnostics.every((d) => d.message !== "stale")).toBe(true);
  });

  it("invalid file shows the source view with a highlighted line, caret and masked secret", async () => {
    const user = userEvent.setup();
    await bootApp({ open: "broken-secrets.json" });
    const view = await screen.findByTestId("source-view");
    expect(within(view).getByRole("alert")).toHaveTextContent(/Expected a comma/);
    expect(view.querySelector("[data-error]")).not.toBeNull();
    expect(view).not.toHaveTextContent("sk-test-0000000000000000");
    expect(view.textContent).toContain("••••");
    await user.click(within(view).getByRole("button", { name: /Reveal masked value/ }));
    await waitFor(() => expect(view).toHaveTextContent("sk-test-0000000000000000"));
    expect(screen.queryByRole("toolbar", { name: "Form tools" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Save changes" })).toBeDisabled();
  });

  it("duplicate keys highlight every occurrence", async () => {
    await bootApp({ open: "duplicate-keys.json" });
    const view = await screen.findByTestId("source-view");
    expect(within(view).getByRole("alert")).toHaveTextContent("Key `a` appears twice");
    expect(view.querySelectorAll("[data-error]").length).toBeGreaterThan(0);
  });

  it("comments in a .json file get the plain-language message", async () => {
    await bootApp({ open: "legacy-comments.json" });
    expect(await screen.findByText("Comments are not allowed in .json files")).toBeInTheDocument();
  });

  it("sidebar flags invalid files with an icon and label, not color alone", async () => {
    await bootApp({ open: "feature-flags.json" });
    await waitFor(() => expect(screen.getByRole("button", { name: "app-config.json, Invalid" })).toBeInTheDocument());
  });

  it("switching away from a draft asks Save / Discard / Cancel", async () => {
    const user = userEvent.setup();
    await bootApp({ open: "feature-flags.json" });
    await toggleDebug(user);
    await user.click(screen.getByRole("button", { name: /^secrets\.json/ }));
    const dialog = await screen.findByRole("dialog", { name: "Unsaved changes" });
    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));
    expect(activeId()).toBe(useApp.getState().files.find((f) => f.displayName === "feature-flags.json")!.id);
    await user.click(screen.getByRole("button", { name: /^secrets\.json/ }));
    await user.click(within(await screen.findByRole("dialog")).getByRole("button", { name: "Discard" }));
    await waitFor(() => expect(useApp.getState().files.find((f) => f.id === activeId())!.displayName).toBe("secrets.json"));
  });

  it("symlink writes need a one-time confirmation", async () => {
    const user = userEvent.setup();
    await bootApp({ flags: ["seeded", "symlink"], open: "linked-config.json" });
    await user.click(await screen.findByRole("switch", { name: "Enabled" }));
    await waitFor(() => expect(doc().dirtyCount).toBe(1));
    await user.click(screen.getByRole("button", { name: "Save changes" }));
    const dialog = await screen.findByRole("dialog", { name: /symbolic link/ });
    await user.click(within(dialog).getByRole("button", { name: "Confirm and save" }));
    await waitFor(() => expect(status()).toHaveTextContent(/Saved/), { timeout: 2000 });
  });

  it("read-only managed files disable editing and say why", async () => {
    await bootApp({ flags: ["seeded", "readonly"], open: "Managed settings" });
    expect(await screen.findByText(/Managed settings are read-only/)).toBeInTheDocument();
    expect(screen.getByRole("textbox", { name: "Default model" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Save changes" })).toBeDisabled();
  });

  it("project-local files carry the version-control note and the R1 repo-root hint", async () => {
    await bootApp({ open: "my-app · Local" });
    expect(await screen.findByText(/Keep this file out of version control/)).toBeInTheDocument();
    expect(screen.getByText(/git repository root/)).toBeInTheDocument();
  });

  it("Ctrl+K finds a field of this file and jumps to it", async () => {
    const user = userEvent.setup();
    await bootApp({ open: "deep.json" });
    await screen.findByRole("button", { name: "A" });
    fireEvent.keyDown(window, { key: "k", ctrlKey: true });
    const box = await screen.findByRole("combobox", { name: "Search settings in this file" });
    await user.type(box, "leaf");
    await user.keyboard("{Enter}");
    await waitFor(() => expect(doc().drillRoot).not.toBe(""));
    expect(await screen.findByRole("navigation", { name: "Breadcrumb" })).toBeInTheDocument();
  });

  it("Settings: shortcuts list, density and theme apply", async () => {
    const user = userEvent.setup();
    await bootApp({ open: "feature-flags.json" });
    await user.click(screen.getByRole("button", { name: "Settings" }));
    const dialog = await screen.findByRole("dialog", { name: "Settings" });
    await user.click(within(dialog).getByRole("radio", { name: "Compact" }));
    await waitFor(() => expect(document.documentElement).toHaveAttribute("data-density", "compact"));
    await user.click(within(dialog).getByRole("radio", { name: "Dark" }));
    await waitFor(() => expect(document.documentElement).toHaveAttribute("data-theme", "dark"));
    await user.click(within(dialog).getByRole("tab", { name: "Shortcuts" }));
    expect(within(dialog).getByText("Ctrl+S")).toBeInTheDocument();
    expect(within(dialog).getByText("Alt+Left")).toBeInTheDocument();
  });

  it("F6 cycles between landmarks", async () => {
    await bootApp({ open: "feature-flags.json" });
    const order = ["banner", "navigation", "main", "status"];
    (document.querySelector("[data-landmark=banner]") as HTMLElement).focus();
    for (const expected of order.slice(1)) {
      fireEvent.keyDown(window, { key: "F6" });
      expect((document.activeElement as HTMLElement).dataset.landmark).toBe(expected);
    }
  });
});
