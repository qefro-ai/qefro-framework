import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ActionBar } from "./ActionBar";
import type { EntityAction } from "../../sdk/client";

describe("ActionBar", () => {
  it("confirms before running an operation", async () => {
    const onAction = vi.fn();
    const actions: EntityAction[] = [
      {
        name: "complete",
        label: "Complete Order",
        confirmation_message: "This will complete the order and create a follow-up task.",
      },
    ];
    render(<ActionBar actions={actions} onAction={onAction} />);
    await userEvent.click(screen.getByRole("button", { name: "Complete Order" }));
    expect(onAction).not.toHaveBeenCalled();
    expect(screen.getByText(/follow-up task/i)).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Confirm" }));
    expect(onAction).toHaveBeenCalledWith("complete", actions[0], {});
  });

  it("collects input_schema fields", async () => {
    const onAction = vi.fn();
    const actions: EntityAction[] = [
      {
        name: "convert",
        label: "Convert",
        input_schema: {
          type: "object",
          properties: { note: { type: "string", title: "Handoff note" } },
        },
      },
    ];
    render(<ActionBar actions={actions} onAction={onAction} />);
    await userEvent.click(screen.getByRole("button", { name: "Convert" }));
    await userEvent.type(screen.getByPlaceholderText("Handoff note"), "Call tomorrow");
    await userEvent.click(screen.getByRole("button", { name: "Confirm" }));
    expect(onAction).toHaveBeenCalledWith("convert", actions[0], { note: "Call tomorrow" });
  });
});
