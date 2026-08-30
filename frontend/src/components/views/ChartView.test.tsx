import { render, screen, waitFor } from "@testing-library/react";
import ChartView from "./ChartView";
import { api } from "../../api";
import type { CollectionViewProps } from "../../views/registry";

const props: CollectionViewProps = {
  meta: {
    entity: "Order",
    label: "Order",
    label_plural: "Orders",
    slug: "orders",
    searchable: true,
    fields: [{ name: "status", type: "enum", label: "Status", required: false, list: true, form: true, filter: true, searchable: false, readonly: false, widget: "status" }],
    views: { chart: { type: "bar", dimension: "status", measure: { aggregation: "count" } } },
  },
  entities: [],
  slug: "orders",
  rows: [],
  total: 0,
  loading: false,
  onReload: () => undefined,
  onError: () => undefined,
};

describe("ChartView", () => {
  afterEach(() => vi.restoreAllMocks());

  it("loads series from aggregates", async () => {
    vi.spyOn(api, "aggregates").mockResolvedValue({
      entity: "Order",
      group_by: "status",
      metric: "count",
      series: [{ label: "Preparing", value: 7 }],
    });
    render(<ChartView {...props} />);
    await waitFor(() => expect(screen.getByRole("img")).toBeInTheDocument());
    expect(screen.getByText("Preparing")).toBeInTheDocument();
  });

  it("shows empty state", async () => {
    vi.spyOn(api, "aggregates").mockResolvedValue({
      entity: "Order",
      group_by: "status",
      metric: "count",
      series: [],
    });
    render(<ChartView {...props} />);
    expect(await screen.findByText("No chart data")).toBeInTheDocument();
  });
});
