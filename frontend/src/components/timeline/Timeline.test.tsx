import { render, screen } from "@testing-library/react";
import { Timeline } from "./Timeline";
import { ActionBar } from "../actions/ActionBar";
import { AttachmentsPanel } from "../attachments/AttachmentsPanel";

describe("Timeline", () => {
  it("renders permitted activity", () => {
    render(
      <Timeline
        items={[
          { action: "created", actor: "Website", created_at: "2026-08-16T09:15:00Z", summary: "Reservation created" },
          { action: "transition", actor: "Manager", created_at: "2026-08-16T10:42:00Z", summary: "Reservation confirmed" },
        ]}
        timezone="UTC"
      />,
    );
    expect(screen.getByText("Reservation created")).toBeInTheDocument();
    expect(screen.getByText("by Manager")).toBeInTheDocument();
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
});

describe("AttachmentsPanel", () => {
  it("lists files without storage keys", () => {
    render(
      <AttachmentsPanel
        slug="orders"
        id="1"
        items={[{ id: "a1", filename: "invoice.pdf", content_type: "application/pdf" }]}
        onChanged={() => undefined}
      />,
    );
    expect(screen.getByText("invoice.pdf")).toBeInTheDocument();
    expect(screen.queryByText(/s3|storage|tenant/i)).not.toBeInTheDocument();
  });
});
