import { render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import Dashboard from "./Dashboard";
import { api } from "../api";
import type { UiEntity } from "../metadata/types";

vi.mock("../realtime", () => ({ useRealtime: () => undefined }));

const entities: UiEntity[] = [
  {
    entity: "Order",
    label: "Order",
    label_plural: "Orders",
    slug: "orders",
    searchable: true,
    fields: [],
    standalone: true,
  },
];

describe("Dashboard widgets", () => {
  afterEach(() => vi.restoreAllMocks());

  it("renders kpi, chart, activity, and hides empty", async () => {
    vi.spyOn(api, "dashboards").mockResolvedValue({
      dashboards: [{ name: "ops", label: "Ops" }],
    });
    vi.spyOn(api, "dashboard").mockResolvedValue({
      name: "ops",
      label: "Restaurant",
      cards: [
        { title: "Ready orders", entity: "Order", metric: "count", kind: "kpi", value: 4 },
        {
          title: "Orders by status",
          entity: "Order",
          metric: "count",
          kind: "workflow",
          chart: "donut",
          group_by: "status",
          value: 4,
          series: [{ label: "Preparing", value: 4 }],
        },
        {
          title: "Recent activity",
          entity: "Order",
          metric: "count",
          kind: "activity",
          value: 1,
          items: [{ id: "1", message: "Order #1042 → Ready", created_at: "2026-08-30T10:42:00Z" }],
        },
      ],
    });
    render(
      <MemoryRouter>
        <Dashboard entities={entities} config={null} />
      </MemoryRouter>,
    );
    await waitFor(() => expect(screen.getByText("Restaurant")).toBeInTheDocument());
    expect(screen.getByText("Ready orders")).toBeInTheDocument();
    expect(screen.getByText("4")).toBeInTheDocument();
    expect(screen.getByText("Orders by status")).toBeInTheDocument();
    expect(screen.getByText("Recent activity")).toBeInTheDocument();
    expect(screen.getByText(/Order #1042/)).toBeInTheDocument();
  });

  it("shows empty state when no dashboard is configured", async () => {
    vi.spyOn(api, "dashboards").mockResolvedValue({ dashboards: [] });
    render(
      <MemoryRouter>
        <Dashboard entities={entities} config={null} />
      </MemoryRouter>,
    );
    expect(await screen.findByText("No dashboard is configured")).toBeInTheDocument();
  });
});
