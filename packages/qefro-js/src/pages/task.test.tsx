import type { ReactElement } from "react";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createMemoryRouter, Outlet, RouterProvider } from "react-router-dom";
import { api } from "../sdk/client";
import EntityDetail from "./EntityDetail";
import { BreadcrumbRecordProvider } from "../components/shell/breadcrumbContext";
import { TenantThemeContext } from "../metadata/context";
import type { UiEntity, UiField } from "../metadata/types";
import "../widgets";

function field(over: Partial<UiField> & { name: string }): UiField {
  return {
    type: "string",
    label: over.label ?? over.name,
    required: false,
    list: true,
    form: true,
    form_visible: true,
    filter: false,
    searchable: false,
    readonly: false,
    widget: "text",
    ...over,
  };
}

function shell(children: ReactElement) {
  return (
    <TenantThemeContext.Provider value={{ timezone: "UTC", locale: "en-US", currency: "USD" }}>
      <BreadcrumbRecordProvider>{children}</BreadcrumbRecordProvider>
    </TenantThemeContext.Provider>
  );
}

function wrap(ui: ReactElement, path: string) {
  const router = createMemoryRouter(
    [
      {
        element: shell(<Outlet />),
        children: [
          { path: "/:slug", element: ui },
          { path: "/:slug/new", element: ui },
          { path: "/:slug/:id", element: ui },
        ],
      },
    ],
    { initialEntries: [path] },
  );
  return render(<RouterProvider router={router} />);
}

const customer: UiEntity = {
  entity: "Customer",
  label: "Customer",
  label_plural: "Customers",
  slug: "customers",
  searchable: true,
  display_field: "name",
  standalone: true,
  permissions: { list: true, create: true, read: true, update: true, delete: true },
  fields: [
    field({ name: "name", label: "Name" }),
    field({
      name: "tasks",
      label: "Tasks",
      type: "relation",
      widget: "relation",
      relation: "Task",
      relation_kind: "one_to_many",
      inverse_field: "entity_id",
    }),
  ],
};

const task: UiEntity = {
  entity: "Task",
  label: "Task",
  label_plural: "Tasks",
  slug: "tasks",
  searchable: true,
  display_field: "title",
  standalone: true,
  workflow: "task",
  permissions: { list: true, create: true, read: true, update: true, delete: true },
  fields: [
    field({ name: "title", label: "Title", required: true }),
    field({
      name: "entity_id",
      label: "Related record",
      type: "uuid",
      widget: "relation",
    }),
  ],
};

describe("Task generic UI", () => {
  afterEach(() => vi.restoreAllMocks());

  it("prefills Add Task from related-record filters without entity-specific chrome", async () => {
    vi.spyOn(api, "get").mockResolvedValue({
      id: "c1",
      name: "Ahmed Khan",
      _related: {
        tasks: {
          entity: "Task",
          slug: "tasks",
          label: "Tasks",
          items: [{ id: "t1", title: "Call customer", status: "Open" }],
          total: 1,
          columns: ["title", "status"],
          filters: [{ field: "entity_type", value: "Customer" }],
        },
      },
    });
    vi.spyOn(api, "audit").mockResolvedValue({ items: [] });
    vi.spyOn(api, "activity").mockResolvedValue({ items: [] });
    wrap(<EntityDetail entities={[customer, task]} />, "/customers/c1");
    await waitFor(() => expect(screen.getByText(/Customer Ahmed Khan/)).toBeInTheDocument());
    await userEvent.click(screen.getByRole("tab", { name: "Related records" }));
    expect(screen.getByText("Call customer")).toBeInTheDocument();
    const add = screen.getByRole("link", { name: "Add" });
    expect(add).toHaveAttribute("href", "/tasks/new?entity_id=c1&entity_type=Customer");
  });

  it("opens the related record from Task detail via metadata expansion", async () => {
    vi.spyOn(api, "get").mockResolvedValue({
      id: "t1",
      title: "Call customer",
      status: "Open",
      entity_type: "Customer",
      entity_id: "c1",
      _expanded: {
        entity_id: { id: "c1", label: "Ahmed Khan", slug: "customers", entity: "Customer" },
      },
      _workflow: { current: "Open", transitions: [{ name: "start", label: "Start" }] },
    });
    vi.spyOn(api, "audit").mockResolvedValue({ items: [] });
    vi.spyOn(api, "activity").mockResolvedValue({ items: [] });
    wrap(<EntityDetail entities={[customer, task]} />, "/tasks/t1");
    await waitFor(() => expect(screen.getByText(/Task Call customer/)).toBeInTheDocument());
    expect(screen.getByRole("link", { name: "Ahmed Khan" })).toHaveAttribute("href", "/customers/c1");
    expect(screen.getByRole("button", { name: "Start" })).toBeInTheDocument();
  });
});
