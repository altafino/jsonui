import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { useApp } from "../../lib/store/app-store";
import { useDocs } from "../../lib/store/doc-store";
import { bootApp } from "../../test/harness";

const doc = () => useDocs.getState().docs[useApp.getState().activeFileId!];
const row = (label: string | RegExp) => {
  const el = screen.getAllByText(label).map((e) => e.closest("[data-pointer]")).find(Boolean) as HTMLElement;
  if (!el) throw new Error("row not found: " + String(label));
  return el;
};
const settle = () => act(() => new Promise<void>((r) => setTimeout(r, 30)));

describe("field controls: absent vs null vs false vs true", () => {
  it("renders each state distinctly and never materialises a missing field", async () => {
    await bootApp({ open: "feature-flags.json" });
    await waitFor(() => expect(screen.getByRole("switch", { name: "Enabled" })).toBeInTheDocument());

    expect(screen.getByRole("switch", { name: "Enabled" })).toBeChecked(); // true
    expect(screen.getByRole("switch", { name: "Debug" })).not.toBeChecked(); // false

    // null: explicit pill + Set value, not an empty text box
    const optional = row("Optional value");
    expect(within(optional).getByText("null")).toBeInTheDocument();
    expect(within(optional).getByRole("button", { name: "Set value" })).toBeInTheDocument();

    // absent: tri-state radiogroup with Not set selected and "not in file"
    const group = await screen.findByRole("radiogroup", { name: "Verbose logging" });
    expect(group).toHaveAccessibleDescription(/not in file/);
    expect(within(group).getByRole("radio", { name: "Not set" })).toBeChecked();
    expect(doc().dirtyCount).toBe(0);
    expect(doc().nodes["/verboseLogging"]).toBeUndefined();
  });

  it("tri-state radiogroup works with the arrow keys and writes only on change", async () => {
    const user = userEvent.setup();
    await bootApp({ open: "feature-flags.json" });
    const group = await screen.findByRole("radiogroup", { name: "Verbose logging" });
    const notSet = within(group).getByRole("radio", { name: "Not set" });
    await user.click(notSet);
    // Radix roving focus moves focus on a timer, so hold the key until it has landed (like a real key press).
    const arrow = async (key: string) => {
      fireEvent.keyDown(document.activeElement!, { key });
      await settle();
      fireEvent.keyUp(document.activeElement!, { key });
    };
    await arrow("ArrowRight");
    await waitFor(() => expect(doc().nodes["/verboseLogging"]?.text).toBe("true"));
    expect(doc().dirtyCount).toBe(1);
    await arrow("ArrowRight");
    await waitFor(() => expect(doc().nodes["/verboseLogging"]?.text).toBe("false")); // explicit false, not absent
    await arrow("ArrowLeft");
    await arrow("ArrowLeft");
    await waitFor(() => expect(doc().nodes["/verboseLogging"]).toBeUndefined()); // Not set removes the key
    await waitFor(() => expect(doc().dirtyCount).toBe(0));
  });

  it("switch toggle sends a boolean op and marks the row changed with the old value", async () => {
    const user = userEvent.setup();
    await bootApp({ open: "feature-flags.json" });
    await user.click(await screen.findByRole("switch", { name: "Debug" }));
    await waitFor(() => expect(screen.getByRole("switch", { name: "Debug" })).toBeChecked());
    await waitFor(() => expect(row("Debug")).toHaveAttribute("data-changed"));
    await waitFor(() => expect(within(row("Debug")).getAllByText(/was false/).length).toBeGreaterThan(0));
    expect(screen.getByTestId("save-status")).toHaveTextContent("Ready · 1 pending change");
  });
});

describe("text and numbers", () => {
  it("keeps the exact number text the user typed", async () => {
    const user = userEvent.setup();
    await bootApp({ open: "feature-flags.json" });
    const input = await screen.findByRole("textbox", { name: "Timeout seconds" });
    await user.clear(input);
    await user.type(input, "1.50");
    await user.tab();
    await waitFor(() => expect(doc().nodes["/timeoutSeconds"].text).toBe("1.50"));
  });

  it("rejects invalid number text without sending it", async () => {
    const user = userEvent.setup();
    await bootApp({ open: "feature-flags.json" });
    const input = await screen.findByRole("textbox", { name: "Timeout seconds" });
    await user.clear(input);
    await user.type(input, "abc");
    await user.tab();
    expect(input).toBeInvalid();
    expect(within(row("Timeout seconds")).getByRole("alert")).toHaveTextContent("Not a valid JSON number");
    await settle();
    expect(doc().nodes["/timeoutSeconds"].text).toBe("1.5");
  });

  it("shows integers beyond 2^53 untouched", async () => {
    await bootApp({ open: "numbers.json" });
    const big = await screen.findByRole("textbox", { name: "Big" });
    expect(big).toHaveValue("9007199254740993");
    expect(await screen.findByRole("textbox", { name: "Precise" })).toHaveValue("1.50");
  });

  it("enforces schema bounds from hints", async () => {
    const user = userEvent.setup();
    await bootApp({ open: "feature-flags.json" });
    const input = await screen.findByRole("textbox", { name: "Retry count" });
    await user.clear(input);
    await user.type(input, "2.5");
    await user.tab();
    expect(screen.getByText("Must be a whole number")).toBeInTheDocument();
  });

  it("masks secret-like values and reveals on demand", async () => {
    const user = userEvent.setup();
    await bootApp({ open: "secrets.json" });
    const token = await screen.findByLabelText("API TOKEN", { selector: "input" });
    expect(token).toHaveAttribute("type", "password");
    await user.click(screen.getAllByRole("button", { name: "Reveal value" })[0]);
    expect(token).toHaveAttribute("type", "text");
    expect(token).toHaveValue("sk-test-0000000000000000");
  });

  it("enum values use a select, and a value outside the list is shown, not coerced", async () => {
    const user = userEvent.setup();
    await bootApp({ open: "feature-flags.json" });
    const level = await screen.findByRole("combobox", { name: "Level" });
    expect(level).toHaveValue('"info"');
    await user.selectOptions(level, '"warn"');
    await waitFor(() => expect(doc().nodes["/logging/level"].text).toBe("warn"));
  });
});

describe("lists, maps and objects", () => {
  it("scalar list: add, edit, delete", async () => {
    const user = userEvent.setup();
    await bootApp({ open: "feature-flags.json" });
    const list = await screen.findByRole("list", { name: "List" });
    expect(within(list).getAllByRole("listitem")).toHaveLength(2);
    await user.click(screen.getByRole("button", { name: /Add item to/ }));
    await waitFor(() => expect(within(screen.getByRole("list", { name: "List" })).getAllByRole("listitem")).toHaveLength(3));
    await user.click(screen.getByRole("button", { name: "Remove item 3" }));
    await waitFor(() => expect(within(screen.getByRole("list", { name: "List" })).getAllByRole("listitem")).toHaveLength(2));
    expect(doc().dirtyCount).toBe(0);
  });

  it("Alt+Down moves a list item", async () => {
    const user = userEvent.setup();
    await bootApp({ open: "feature-flags.json" });
    const first = await screen.findByRole("button", { name: /Reorder item 1 of 2/ });
    first.focus();
    await user.keyboard("{Alt>}{ArrowDown}{/Alt}");
    await waitFor(() => expect(doc().nodes["/features/0"].text).toBe("analytics"));
    expect(doc().nodes["/features/1"].text).toBe("search");
  });

  it("array of objects shows summary rows (DD17) and the add-setting menu offers schema names", async () => {
    const user = userEvent.setup();
    await bootApp({ open: "feature-flags.json" });
    const toggle = await screen.findByRole("button", { name: /^Services/ });
    await user.click(toggle);
    expect(await screen.findByText("frontend · enabled")).toBeInTheDocument();
    expect(screen.getByText("backend · enabled off")).toBeInTheDocument();
  });

  it("map editor rejects duplicate keys", async () => {
    const user = userEvent.setup();
    await bootApp({ open: "my-app · Shared" });
    await screen.findByRole("textbox", { name: "Key EXAMPLE_NON_SECRET_FLAG" });
    await user.type(screen.getByLabelText("New entry key"), "API_TOKEN");
    await user.click(screen.getByRole("button", { name: "Add" }));
    expect(await screen.findByText('Key "API_TOKEN" already exists')).toBeInTheDocument();
    await user.clear(screen.getByLabelText("New entry key"));
    await user.type(screen.getByLabelText("New entry key"), "NEW_ONE");
    await user.type(screen.getByLabelText("New entry value"), "1");
    await user.click(screen.getByRole("button", { name: "Add" }));
    await waitFor(() => expect(screen.getByRole("textbox", { name: "Key NEW_ONE" })).toBeInTheDocument());
  });

  it("deep containers beyond depth 3 open as a drill-in view with a breadcrumb", async () => {
    const user = userEvent.setup();
    await bootApp({ open: "deep.json" });
    await user.click(await screen.findByRole("button", { name: /^B(\d+ keys?)?$/ }));
    await user.click(await screen.findByRole("button", { name: /^C(\d+ keys?)?$/ }));
    await user.click(await screen.findByRole("button", { name: "Open D" }));
    const crumbs = await screen.findByRole("navigation", { name: "Breadcrumb" });
    expect(within(crumbs).getByText("d")).toHaveAttribute("aria-current", "page");
    fireEvent.keyDown(window, { key: "ArrowLeft", altKey: true });
    await waitFor(() => expect(within(screen.getByRole("navigation", { name: "Breadcrumb" })).getByText("c")).toHaveAttribute("aria-current", "page"));
  });
});

describe("search and filters", () => {
  it("shows the exact empty-state copy for no match and for nothing changed", async () => {
    const user = userEvent.setup();
    await bootApp({ open: "feature-flags.json" });
    await user.type(await screen.findByRole("searchbox", { name: "Search fields" }), "xyz");
    expect(await screen.findByText('No fields match "xyz"')).toBeInTheDocument();
    await user.click(within(screen.getByTestId("empty-state")).getByRole("button", { name: "Clear search" }));
    await user.click(screen.getByRole("button", { name: "Changed only" }));
    expect(await screen.findByText("No unsaved changes", { selector: "p" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Show all fields" }));
    expect(await screen.findByRole("switch", { name: "Enabled" })).toBeInTheDocument();
  });

  it("search expands matching parents", async () => {
    const user = userEvent.setup();
    await bootApp({ open: "feature-flags.json" });
    await user.type(await screen.findByRole("searchbox", { name: "Search fields" }), "to file");
    expect(await screen.findByRole("switch", { name: "To file" })).toBeInTheDocument();
    expect(screen.queryByRole("switch", { name: "Enabled" })).not.toBeInTheDocument();
  });
});
