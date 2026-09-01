import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { api } from "../../api";
import type { UiEntity, UiField } from "../../metadata/types";
import KanbanView from "./KanbanView";

function field(over: Partial<UiField> & { name: string }): UiField {
  return {
    type: "string",
    label: over.label ?? over.name,
    required: false,
    list: true,
    form: true,
    filter: false,
    searchable: false,
    readonly: false,
    widget: "text",
    ...over,
  };
}

const reservation: UiEntity = {
  entity: "Reservation",
  label: "Reservation",
  label_plural: "Reservations",
  slug: "reservations",
  searchable: true,
  workflow: "reservation",
  views: { kanban: { group_by: "status", card: { title: "guest_name", subtitle: "reservation_time" } } },
  fields: [
    field({ name: "guest_name", label: "Name" }),
    field({
      name: "status",
      type: "enum",
      widget: "status",
      enum_values: ["Pending", "Confirmed", "Completed"],
    }),
  ],
};

const row = {
  id: "r1",
  guest_name: "Ahmed",
  reservation_time: "19:00",
  status: "Pending",
  _workflow: {
    current: "Pending",
    transitions: [{ name: "confirm", label: "Confirm", from: "Pending", to: "Confirmed" }],
  },
};

describe("KanbanView", () => {
  beforeEach(() => {
    vi.spyOn(api, "transition").mockResolvedValue({ id: "r1" });
    vi.spyOn(api, "update").mockResolvedValue({ id: "r1" });
  });

  afterEach(() => vi.restoreAllMocks());

  it("moves a workflow card through EntityService transition, never PATCH status", async () => {
    render(
      <MemoryRouter>
        <KanbanView
          meta={reservation}
          entities={[reservation]}
          slug="reservations"
          rows={[row]}
          total={1}
          loading={false}
          onReload={() => undefined}
          onError={() => undefined}
        />
      </MemoryRouter>,
    );
    const card = screen.getByText("Ahmed").closest("article")!;
    const confirmed = screen.getByText("Confirmed").closest("section")!;
    fireEvent.drop(confirmed, { dataTransfer: { getData: () => "r1" } });
    await waitFor(() => expect(api.transition).toHaveBeenCalledWith("reservations", "r1", "confirm"));
    expect(api.update).not.toHaveBeenCalled();
    expect(card).toBeInTheDocument();
  });

  it("rejects a drop with no workflow transition", async () => {
    const onError = vi.fn();
    render(
      <MemoryRouter>
        <KanbanView
          meta={reservation}
          entities={[reservation]}
          slug="reservations"
          rows={[row]}
          total={1}
          loading={false}
          onReload={() => undefined}
          onError={onError}
        />
      </MemoryRouter>,
    );
    const completed = screen.getByText("Completed").closest("section")!;
    fireEvent.drop(completed, { dataTransfer: { getData: () => "r1" } });
    await waitFor(() =>
      expect(onError).toHaveBeenCalledWith("Cannot move reservation from Pending to Completed."),
    );
    expect(api.transition).not.toHaveBeenCalled();
    expect(api.update).not.toHaveBeenCalled();
  });
});
