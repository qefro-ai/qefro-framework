import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { MemoryRouter, Route, Routes, useLocation } from "react-router-dom";
import { api } from "../../api";
import { TenantThemeContext } from "../../metadata/context";
import type { UiEntity, UiField } from "../../metadata/types";
import CalendarView from "./CalendarView";

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
  views: { calendar: { start: "reservation_date", time: "reservation_time", title: "guest_name" } },
  fields: [
    field({ name: "guest_name" }),
    field({ name: "reservation_date", type: "date", widget: "date" }),
    field({ name: "reservation_time", type: "time", widget: "time" }),
    field({ name: "status", type: "enum", widget: "status" }),
  ],
};

function Probe() {
  const loc = useLocation();
  return <div data-testid="loc">{`${loc.pathname}${loc.search}`}</div>;
}

function wrap(ui: ReactNode, path = "/reservations?view=calendar&cal=day&cursor=2026-08-16") {
  return render(
    <TenantThemeContext.Provider value={{ timezone: "UTC", locale: "en", currency: "USD" }}>
      <MemoryRouter initialEntries={[path]}>
        <Routes>
          <Route
            path="/reservations"
            element={
              <>
                {ui}
                <Probe />
              </>
            }
          />
          <Route path="/reservations/new" element={<Probe />} />
          <Route path="/reservations/:id" element={<Probe />} />
        </Routes>
      </MemoryRouter>
    </TenantThemeContext.Provider>,
  );
}

describe("CalendarView", () => {
  beforeEach(() => {
    vi.spyOn(api, "update").mockResolvedValue({ id: "r1" });
  });
  afterEach(() => vi.restoreAllMocks());

  it("opens the generic form with the selected slot as defaults", () => {
    wrap(
      <CalendarView
        meta={reservation}
        entities={[reservation]}
        slug="reservations"
        rows={[]}
        total={0}
        loading={false}
        onReload={() => undefined}
        onError={() => undefined}
      />,
    );
    fireEvent.click(screen.getAllByRole("button", { name: "09:00" })[0]);
    expect(screen.getByTestId("loc").textContent).toContain("/reservations/new?");
    expect(screen.getByTestId("loc").textContent).toContain("reservation_date=");
    expect(screen.getByTestId("loc").textContent).toContain("reservation_time=09%3A00");
  });

  it("opens the generic detail view when an event is clicked", () => {
    wrap(
      <CalendarView
        meta={reservation}
        entities={[reservation]}
        slug="reservations"
        rows={[{ id: "r1", guest_name: "Ahmed", reservation_date: "2026-08-16", reservation_time: "09:00" }]}
        total={1}
        loading={false}
        onReload={() => undefined}
        onError={() => undefined}
      />,
    );
    fireEvent.click(screen.getByText(/Ahmed/));
    expect(screen.getByTestId("loc")).toHaveTextContent("/reservations/r1");
  });

  it("refuses to reschedule a locked or readonly record", async () => {
    const onError = vi.fn();
    const locked: UiEntity = {
      ...reservation,
      fields: reservation.fields.map((f) => (f.name === "reservation_date" ? { ...f, readonly: true } : f)),
    };
    wrap(
      <CalendarView
        meta={locked}
        entities={[locked]}
        slug="reservations"
        rows={[{ id: "r1", guest_name: "Ahmed", reservation_date: "2026-08-16", reservation_time: "09:00", status: "Seated" }]}
        total={1}
        loading={false}
        onReload={() => undefined}
        onError={onError}
      />,
    );
    const day = screen.getByText(/Aug 16/i).closest("section")!;
    fireEvent.drop(day, { dataTransfer: { getData: () => "r1" } });
    await waitFor(() => expect(onError).toHaveBeenCalledWith("Cannot reschedule this record."));
    expect(api.update).not.toHaveBeenCalled();
  });

  it("switches to agenda and today", () => {
    wrap(
      <CalendarView
        meta={reservation}
        entities={[reservation]}
        slug="reservations"
        rows={[]}
        total={0}
        loading={false}
        onReload={() => undefined}
        onError={() => undefined}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Agenda" }));
    expect(screen.getByTestId("loc").textContent).toContain("cal=agenda");
    fireEvent.click(screen.getByRole("button", { name: "Today" }));
    expect(screen.getByRole("button", { name: "Today" })).toBeInTheDocument();
  });
});
