import { render, screen } from "@testing-library/react";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { Breadcrumbs } from "./Breadcrumbs";
import { BreadcrumbRecordProvider, useBreadcrumbRecord } from "./breadcrumbContext";
import type { UiEntity } from "../../metadata/types";
import { useEffect } from "react";

const order: UiEntity = {
  entity: "Order",
  label: "Order",
  label_plural: "Orders",
  slug: "orders",
  searchable: true,
  fields: [],
};

const item: UiEntity = {
  entity: "OrderItem",
  label: "Line",
  label_plural: "Lines",
  slug: "order-items",
  searchable: true,
  child_of: "Order",
  fields: [],
};

function Seed({ label, parent }: { label: string; parent?: { slug: string; id: string; label: string; entityLabel: string } }) {
  const { setRecord } = useBreadcrumbRecord();
  useEffect(() => {
    setRecord({ id: "r1", label, parent });
  }, [label, parent, setRecord]);
  return null;
}

describe("Breadcrumbs", () => {
  it("uses the record display label instead of the UUID", () => {
    render(
      <MemoryRouter initialEntries={["/orders/abc-uuid"]}>
        <BreadcrumbRecordProvider>
          <Seed label="SO-1001" />
          <Routes>
            <Route path="/:slug/:id" element={<Breadcrumbs entities={[order]} />} />
          </Routes>
        </BreadcrumbRecordProvider>
      </MemoryRouter>,
    );
    expect(screen.getByText("SO-1001")).toBeInTheDocument();
    expect(screen.queryByText("abc-uuid")).not.toBeInTheDocument();
  });

  it("shows parent / record / child crumbs for child_of entities", () => {
    render(
      <MemoryRouter initialEntries={["/order-items/line-1"]}>
        <BreadcrumbRecordProvider>
          <Seed
            label="Widget"
            parent={{ slug: "orders", id: "o1", label: "SO-1001", entityLabel: "Order" }}
          />
          <Routes>
            <Route path="/:slug/:id" element={<Breadcrumbs entities={[order, item]} />} />
          </Routes>
        </BreadcrumbRecordProvider>
      </MemoryRouter>,
    );
    expect(screen.getByText("Order")).toBeInTheDocument();
    expect(screen.getByText("SO-1001")).toBeInTheDocument();
    expect(screen.getByText("Widget").closest(".crumb")).toHaveClass("is-current");
    expect(screen.getByText("SO-1001").closest(".crumb")).toHaveClass("is-keep");
  });
});
