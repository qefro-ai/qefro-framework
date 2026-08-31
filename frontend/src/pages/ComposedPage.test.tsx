import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import ComposedPage from "./ComposedPage";
import { api } from "../api";
import { TenantThemeContext } from "../metadata/context";
import type { UiEntity } from "../metadata/types";

vi.mock("../realtime", () => ({ useRealtime: () => undefined }));

const entities: UiEntity[] = [
  {
    entity: "Order",
    label: "Order",
    label_plural: "Orders",
    slug: "orders",
    searchable: true,
    permissions: { list: true, create: true, read: true, update: true, delete: true },
    fields: [
      {
        name: "doc_no",
        type: "string",
        label: "Number",
        required: false,
        list: true,
        form: true,
        filter: true,
        searchable: true,
        widget: "text",
        readonly: false,
      },
      {
        name: "status",
        type: "enum",
        label: "Status",
        required: false,
        list: true,
        form: true,
        filter: true,
        searchable: false,
        widget: "select",
        readonly: false,
        enum_values: ["Preparing", "Ready"],
      },
    ],
    standalone: true,
    views: { default: "kanban", kanban: { enabled: true, group_by: "status" } },
  },
  {
    entity: "Reservation",
    label: "Reservation",
    label_plural: "Reservations",
    slug: "reservations",
    searchable: true,
    permissions: { list: true, create: true, read: true, update: true, delete: false },
    fields: [
      {
        name: "guest_name",
        type: "string",
        label: "Guest",
        required: false,
        list: true,
        form: true,
        filter: false,
        searchable: true,
        widget: "text",
        readonly: false,
      },
    ],
    standalone: true,
  },
];

function renderPage(path = "/pages/restaurant-operations") {
  return render(
    <TenantThemeContext.Provider value={{ timezone: "UTC", locale: "en-US", currency: "USD" }}>
      <MemoryRouter initialEntries={[path]}>
        <Routes>
          <Route path="/pages/:name" element={<ComposedPage entities={entities} />} />
        </Routes>
      </MemoryRouter>
    </TenantThemeContext.Provider>,
  );
}

describe("ComposedPage", () => {
  afterEach(() => vi.restoreAllMocks());

  it("renders layout, widgets, entity views, empty and error sections", async () => {
    vi.spyOn(api, "page").mockResolvedValue({
      name: "restaurant-operations",
      label: "Restaurant Operations",
      slug: "restaurant-operations",
      layout: "grid",
      tabs: [
        { name: "overview", label: "Overview" },
        { name: "kitchen", label: "Kitchen" },
      ],
      actions: [{ entity: "Order", action: "create", label: "New Order" }],
      sections: [
        {
          title: "Today's Sales",
          kind: "widget",
          dashboard: "restaurant-ops",
          widget: "Today's sales",
          size: "md",
        },
        {
          title: "Kitchen",
          kind: "entity_view",
          entity: "Order",
          view: "list",
          query: "status=Preparing",
          size: "xl",
          tab: "kitchen",
        },
        {
          title: "Reservations",
          kind: "entity_view",
          entity: "Reservation",
          view: "list",
          size: "md",
        },
      ],
    });
    vi.spyOn(api, "dashboard").mockResolvedValue({
      name: "restaurant-ops",
      label: "Ops",
      cards: [{ title: "Today's sales", entity: "Payment", metric: "sum", kind: "kpi", value: 42 }],
    });
    vi.spyOn(api, "list").mockImplementation(async (slug) => {
      if (slug === "reservations") return { items: [], total: 0, page: 1, page_size: 15 };
      return {
        items: [{ id: "o1", doc_no: "1001", status: "Preparing" }],
        total: 1,
        page: 1,
        page_size: 15,
      };
    });

    renderPage();
    await waitFor(() => expect(screen.getByText("Restaurant Operations")).toBeInTheDocument());
    expect(screen.getByText("Today's Sales")).toBeInTheDocument();
    await waitFor(() => expect(screen.getByText(/42/)).toBeInTheDocument());
    expect(screen.getByRole("button", { name: "New Order" })).toBeInTheDocument();
    await waitFor(() => expect(screen.getByText("No reservations")).toBeInTheDocument());
    expect(screen.getByRole("tab", { name: "Overview" })).toBeInTheDocument();

    await userEvent.click(screen.getByRole("tab", { name: "Kitchen" }));
    await waitFor(() => expect(screen.getByText("1001")).toBeInTheDocument());
  });

  it("shows a page-level error without crashing", async () => {
    vi.spyOn(api, "page").mockRejectedValue(new Error("Unable to load data"));
    renderPage();
    await waitFor(() => expect(screen.getByText(/Unable to load data/)).toBeInTheDocument());
    expect(screen.getByRole("button", { name: "Retry" })).toBeInTheDocument();
  });

  it("renders split master-detail selection", async () => {
    vi.spyOn(api, "page").mockResolvedValue({
      name: "customer-workspace",
      label: "Customer Workspace",
      layout: "split",
      context_param: "id",
      sections: [
        { title: "Customers", kind: "entity_view", entity: "Reservation", view: "list", pane: "master" },
      ],
    });
    vi.spyOn(api, "list").mockResolvedValue({
      items: [{ id: "r1", guest_name: "Ada" }],
      total: 1,
      page: 1,
      page_size: 15,
    });
    vi.spyOn(api, "get").mockResolvedValue({ id: "r1", guest_name: "Ada" });
    renderPage("/pages/customer-workspace");
    await waitFor(() => expect(screen.getByText("Customer Workspace")).toBeInTheDocument());
    await waitFor(() => expect(screen.getByText("Ada")).toBeInTheDocument());
    await userEvent.click(screen.getByRole("button", { name: "Ada" }));
    await waitFor(() => expect(api.get).toHaveBeenCalled());
  });
});
