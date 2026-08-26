import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { FilterBar } from "./FilterBar";
import type { UiField } from "../../api";

const fields: UiField[] = [
  {
    name: "status",
    type: "enum",
    label: "Status",
    required: false,
    list: true,
    form: true,
    filter: true,
    searchable: false,
    readonly: false,
    widget: "status",
    enum_values: ["Pending", "Confirmed"],
  },
  {
    name: "reservation_date",
    type: "date",
    label: "Date",
    required: false,
    list: true,
    form: true,
    filter: true,
    searchable: false,
    readonly: false,
    widget: "date",
  },
];

describe("FilterBar", () => {
  beforeEach(() => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => ({
        ok: true,
        status: 200,
        json: async () => ({ items: [] }),
        statusText: "OK",
      })),
    );
  });

  it("adds a filter and exposes date presets", async () => {
    const onChange = vi.fn();
    render(
      <FilterBar
        entity="Reservation"
        fields={fields}
        entities={[]}
        params={new URLSearchParams()}
        onChange={onChange}
      />,
    );
    await userEvent.click(screen.getByText("+ Add filter"));
    await userEvent.click(screen.getByText("Date"));
    expect(screen.getByLabelText("Date preset")).toBeInTheDocument();
    expect(screen.getByText("Today")).toBeInTheDocument();
    await userEvent.click(screen.getByText("Apply"));
  });

  it("shows chips and reset for active filters", async () => {
    const onReplace = vi.fn();
    render(
      <FilterBar
        entity="Reservation"
        fields={fields}
        entities={[]}
        params={new URLSearchParams("status=Pending")}
        onChange={() => undefined}
        onReplace={onReplace}
      />,
    );
    expect(screen.getByLabelText("Active filters")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Reset" }));
    expect(onReplace).toHaveBeenCalled();
    const next = onReplace.mock.calls[0][0] as URLSearchParams;
    expect(next.get("status")).toBeNull();
  });
});
