import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { FilterBar, SavedViewsMenu } from "./FilterBar";
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
    expect(screen.getByRole("dialog", { name: "Add filter" })).toBeInTheDocument();
    await userEvent.click(screen.getByText("Date"));
    expect(screen.getByLabelText("Date preset")).toBeInTheDocument();
    expect(screen.getByText("Today")).toBeInTheDocument();
    await userEvent.click(screen.getByText("Apply"));
    expect(screen.queryByRole("dialog", { name: "Add filter" })).not.toBeInTheDocument();
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
    expect(screen.getByText("Status: Pending")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Reset" })).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Clear Status filter" }));
    expect(onReplace).toHaveBeenCalled();
    const next = onReplace.mock.calls[0][0] as URLSearchParams;
    expect(next.get("status")).toBeNull();
  });

  it("keeps the builder in a popover instead of a stacked form", async () => {
    render(
      <FilterBar
        entity="Reservation"
        fields={fields}
        entities={[]}
        params={new URLSearchParams()}
        onChange={() => undefined}
      />,
    );
    expect(screen.queryByPlaceholderText("Save as…")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Saved filters")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Date operator")).not.toBeInTheDocument();
    await userEvent.click(screen.getByText("+ Add filter"));
    await userEvent.click(screen.getByText("Date"));
    expect(screen.getByLabelText("Date operator")).toBeInTheDocument();
  });

  it("applies an enum filter when the value changes", async () => {
    const onReplace = vi.fn();
    render(
      <FilterBar
        entity="Reservation"
        fields={fields}
        entities={[]}
        params={new URLSearchParams()}
        onChange={() => undefined}
        onReplace={onReplace}
      />,
    );
    await userEvent.click(screen.getByText("+ Add filter"));
    await userEvent.click(screen.getByRole("button", { name: "Status" }));
    await userEvent.selectOptions(screen.getByDisplayValue("Any"), "Confirmed");
    expect(onReplace).toHaveBeenCalled();
    const next = onReplace.mock.calls[0][0] as URLSearchParams;
    expect(next.get("status")).toBe("Confirmed");
  });
});

describe("SavedViewsMenu", () => {
  beforeEach(() => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => ({
        ok: true,
        status: 200,
        json: async () => ({ items: [{ id: "v1", name: "Tonight", query: { status: "Pending" } }] }),
        statusText: "OK",
      })),
    );
  });

  it("hides save-as until filters are active", async () => {
    const { rerender } = render(
      <SavedViewsMenu
        entity="Reservation"
        params={new URLSearchParams()}
        canSave={false}
        onChange={() => undefined}
      />,
    );
    expect(await screen.findByText("Tonight")).toBeInTheDocument();
    expect(screen.queryByPlaceholderText("Save as…")).not.toBeInTheDocument();
    expect(screen.getByText(/Apply a search or filter/i)).toBeInTheDocument();
    rerender(
      <SavedViewsMenu
        entity="Reservation"
        params={new URLSearchParams("status=Pending")}
        canSave
        onChange={() => undefined}
      />,
    );
    expect(screen.getByPlaceholderText("Save as…")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Save" })).toBeInTheDocument();
  });
});
