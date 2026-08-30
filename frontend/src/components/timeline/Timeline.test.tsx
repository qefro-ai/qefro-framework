import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Timeline } from "./Timeline";
import { ActionBar } from "../actions/ActionBar";
import { AttachmentsPanel } from "../attachments/AttachmentsPanel";

describe("Timeline", () => {
  it("renders permitted activity grouped by day", () => {
    render(
      <Timeline
        items={[
          {
            action: "created",
            actor: "Website",
            created_at: "2026-08-16T09:15:00Z",
            summary: "Reservation created",
          },
          {
            activity_type: "workflow_transition",
            actor_name: "Manager",
            created_at: "2026-08-16T10:42:00Z",
            message: "Reservation moved Pending → Confirmed",
          },
        ]}
        timezone="UTC"
      />,
    );
    expect(screen.getByText("Reservation created")).toBeInTheDocument();
    expect(screen.getByText("Reservation moved Pending → Confirmed")).toBeInTheDocument();
    expect(screen.getByText("Manager")).toBeInTheDocument();
    expect(screen.getByText("Website")).toBeInTheDocument();
  });
});

describe("ActionBar", () => {
  it("puts extra actions in More", async () => {
    const onAction = vi.fn();
    render(
      <ActionBar
        actions={[
          { name: "confirm", label: "Confirm" },
          { name: "print", label: "Print" },
          { name: "cancel", label: "Cancel", style: "danger" },
        ]}
        onAction={onAction}
      />,
    );
    expect(screen.getByText("Confirm")).toBeInTheDocument();
    expect(screen.getByText("More")).toBeInTheDocument();
  });

  it("asks for confirmation before a workflow transition", async () => {
    const onTransition = vi.fn();
    render(
      <ActionBar
        actions={[]}
        transitions={[
          {
            name: "cancel",
            label: "Cancel",
            from: "Preparing",
            to: "Cancelled",
            requires_confirmation: true,
            confirmation_message: "Cancel this order?",
          },
        ]}
        onTransition={onTransition}
      />,
    );
    await userEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    expect(screen.getByText("Cancel this order?")).toBeInTheDocument();
    expect(onTransition).not.toHaveBeenCalled();
    await userEvent.click(screen.getByRole("button", { name: "Confirm" }));
    expect(onTransition).toHaveBeenCalledWith("cancel");
  });

  it("does not duplicate a transition already covered by an operation", () => {
    render(
      <ActionBar
        actions={[
          { name: "start_preparation", label: "Start Preparation", workflow_transition: "prepare" },
        ]}
        transitions={[
          { name: "prepare", label: "Start Preparing", from: "Confirmed", to: "Preparing" },
          { name: "ready", label: "Mark Ready", from: "Confirmed", to: "Ready" },
        ]}
        onAction={() => undefined}
      />,
    );
    expect(screen.getByRole("button", { name: "Start Preparation" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Start Preparing" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Mark Ready" })).toBeInTheDocument();
  });
});

describe("AttachmentsPanel", () => {
  it("lists files without storage keys", () => {
    render(
      <AttachmentsPanel
        slug="orders"
        id="1"
        items={[{ id: "a1", filename: "invoice.pdf", content_type: "application/pdf", size: 245000 }]}
        onChanged={() => undefined}
      />,
    );
    expect(screen.getByText("invoice.pdf")).toBeInTheDocument();
    expect(screen.getByText(/KB/)).toBeInTheDocument();
    expect(screen.queryByText(/s3|storage|tenant/i)).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: /\+ Add attachment/ })).toBeInTheDocument();
  });
});
