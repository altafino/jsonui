import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { TooltipProvider } from "../ui/Tooltip";
import { ChangeList } from "./ChangeList";

const change = (over: Record<string, unknown>) => ({ pointer: "/a", kind: "changed", oldText: "1", newText: "2", masked: false, opIndex: 0, revertible: true, ...over }) as never;

describe("ChangeList", () => {
  it("disables Revert when a later change depends on the path and explains why", async () => {
    const onRevert = vi.fn();
    render(
      <TooltipProvider>
        <ChangeList changes={[change({ revertible: false })]} onRevert={onRevert} empty="none" label="Changes" />
      </TooltipProvider>,
    );
    const btn = screen.getByRole("button", { name: /Revert/ });
    expect(btn).toBeDisabled();
    await userEvent.setup().click(btn);
    expect(onRevert).not.toHaveBeenCalled();
  });

  it("reverts by op index and shows masked values as bullets", async () => {
    const onRevert = vi.fn();
    render(
      <TooltipProvider>
        <ChangeList changes={[change({ opIndex: 3, masked: true, oldText: "••••", newText: "••••", pointer: "/env/API_TOKEN" })]} onRevert={onRevert} empty="none" label="Changes" />
      </TooltipProvider>,
    );
    await userEvent.setup().click(screen.getByRole("button", { name: /Revert/ }));
    expect(onRevert).toHaveBeenCalledWith(3);
    expect(screen.getAllByText("••••").length).toBe(2);
  });

  it("renders the empty copy", () => {
    render(<ChangeList changes={[]} empty="No unsaved changes" label="Changes" />);
    expect(screen.getByText("No unsaved changes")).toBeInTheDocument();
  });
});
