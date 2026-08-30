import type { ReactElement } from "react";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { api } from "../sdk/client";
import EntityDetail from "./EntityDetail";
import EntityList from "./EntityList";
import AuditLog from "./AuditLog";
import NotificationBell from "../components/NotificationBell";
import { BreadcrumbRecordProvider } from "../components/shell/breadcrumbContext";
import { TenantThemeContext } from "../metadata/context";
import type { UiEntity, UiField } from "../metadata/types";
import { ApiError } from "../sdk/client";

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

const order: UiEntity = {
  entity: "Order",
  label: "Order",
  label_plural: "Orders",
  slug: "orders",
  searchable: true,
  display_field: "name",
  standalone: true,
  workflow: "order",
  attachments: true,
  capabilities: {
    workflow: true,
    activity: true,
    comments: true,
    attachments: true,
    audit: true,
    relations: true,
    actions: true,
  },
  fields: [
    field({ name: "name", label: "Name" }),
    field({
      name: "status",
      label: "Status",
      widget: "status",
      enum_values: ["Draft", "Submitted", "Approved"],
    }),
  ],
  permissions: { list: true, create: true, read: true, update: true, delete: true },
};

function wrap(ui: ReactElement, path: string) {
  return render(
    <TenantThemeContext.Provider value={{ timezone: "UTC", locale: "en-US", currency: "USD" }}>
      <MemoryRouter initialEntries={[path]}>
        <BreadcrumbRecordProvider>
          <Routes>
            <Route path="/:slug" element={ui} />
            <Route path="/:slug/:id" element={ui} />
            <Route path="/settings/audit" element={ui} />
          </Routes>
        </BreadcrumbRecordProvider>
      </MemoryRouter>
    </TenantThemeContext.Provider>,
  );
}

describe("business object runtime UI", () => {
  afterEach(() => vi.restoreAllMocks());

  it("shows workflow status and transition buttons from metadata", async () => {
    vi.spyOn(api, "get").mockResolvedValue({
      id: "o1",
      name: "#1042",
      status: "Draft",
      _workflow: {
        current: "Draft",
        transitions: [{ name: "submit", label: "Submit", from: "Draft", to: "Submitted" }],
      },
      _actions: [],
    });
    vi.spyOn(api, "activity").mockResolvedValue({ items: [] });
    vi.spyOn(api, "attachments").mockResolvedValue({ items: [] });
    wrap(<EntityDetail entities={[order]} />, "/orders/o1");
    await waitFor(() => expect(screen.getByText(/Order #1042/)).toBeInTheDocument());
    expect(screen.getAllByText("Draft").length).toBeGreaterThan(0);
    expect(screen.getByRole("button", { name: "Submit" })).toBeInTheDocument();
  });

  it("confirms a transition then calls the transition endpoint", async () => {
    vi.spyOn(api, "get").mockResolvedValue({
      id: "o1",
      name: "#1042",
      status: "Preparing",
      _workflow: {
        current: "Preparing",
        transitions: [
          {
            name: "cancel",
            label: "Cancel",
            from: "Preparing",
            to: "Cancelled",
            requires_confirmation: true,
            confirmation_message: "Cancel this order?",
          },
        ],
      },
    });
    vi.spyOn(api, "activity").mockResolvedValue({ items: [] });
    vi.spyOn(api, "attachments").mockResolvedValue({ items: [] });
    const transition = vi.spyOn(api, "transition").mockResolvedValue({ id: "o1", status: "Cancelled" });
    wrap(<EntityDetail entities={[order]} />, "/orders/o1");
    await waitFor(() => expect(screen.getByRole("button", { name: "Cancel" })).toBeInTheDocument());
    await userEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(screen.getByText("Cancel this order?")).toBeInTheDocument();
    expect(transition).not.toHaveBeenCalled();
    await userEvent.click(screen.getByRole("button", { name: "Confirm" }));
    await waitFor(() => expect(transition).toHaveBeenCalledWith("orders", "o1", "cancel"));
  });

  it("deletes a record from an in-app confirm dialog", async () => {
    vi.spyOn(api, "get").mockResolvedValue({
      id: "o1",
      name: "#1042",
      status: "Draft",
      _permissions: { update: true, delete: true },
    });
    vi.spyOn(api, "activity").mockResolvedValue({ items: [] });
    vi.spyOn(api, "attachments").mockResolvedValue({ items: [] });
    const remove = vi.spyOn(api, "remove").mockResolvedValue(undefined);
    wrap(<EntityDetail entities={[order]} />, "/orders/o1");
    await waitFor(() => expect(screen.getByText(/Order #1042/)).toBeInTheDocument());
    await userEvent.click(screen.getByRole("button", { name: "More" }));
    await userEvent.click(screen.getByRole("menuitem", { name: "Delete" }));
    expect(remove).not.toHaveBeenCalled();
    expect(screen.getByRole("dialog", { name: "Delete Order" })).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Delete" }));
    await waitFor(() => expect(remove).toHaveBeenCalledWith("orders", "o1"));
  });

  it("renders timeline comments and empty attachments from generic APIs", async () => {
    vi.spyOn(api, "get").mockResolvedValue({
      id: "o1",
      name: "#1042",
      status: "Submitted",
    });
    vi.spyOn(api, "activity").mockResolvedValue({
      items: [
        {
          id: "a1",
          activity_type: "comment",
          message: "Customer requested window seating.",
          actor_name: "Ahmed Khan",
          created_at: new Date().toISOString(),
        },
      ],
    });
    vi.spyOn(api, "attachments").mockResolvedValue({ items: [] });
    wrap(<EntityDetail entities={[order]} />, "/orders/o1");
    await waitFor(() => expect(screen.getByText(/Order #1042/)).toBeInTheDocument());
    await userEvent.click(screen.getByRole("tab", { name: "Activity" }));
    expect(await screen.findByText("Customer requested window seating.")).toBeInTheDocument();
    expect(screen.getByText("Ahmed Khan")).toBeInTheDocument();
    expect(screen.getByPlaceholderText(/Write a comment/)).toBeInTheDocument();
    await userEvent.click(screen.getByRole("tab", { name: "Attachments" }));
    expect(screen.getByText("No files attached.")).toBeInTheDocument();
  });

  it("hides workflow chrome when the entity has no workflow capability", async () => {
    const note: UiEntity = {
      ...order,
      entity: "Note",
      label: "Note",
      label_plural: "Notes",
      slug: "notes",
      workflow: undefined,
      attachments: false,
      capabilities: { workflow: false, activity: false, comments: false, attachments: false },
      fields: [field({ name: "title", label: "Title" })],
    };
    vi.spyOn(api, "list").mockResolvedValue({
      items: [{ id: "n1", title: "Hello" }],
      total: 1,
      page: 1,
      page_size: 25,
    });
    wrap(<EntityList entities={[note]} />, "/notes");
    await waitFor(() => expect(screen.getByText("Notes")).toBeInTheDocument());
    expect(screen.queryByText("Actions")).not.toBeInTheDocument();
  });

  it("shows list-row workflow actions from _workflow metadata", async () => {
    vi.spyOn(api, "list").mockResolvedValue({
      items: [
        {
          id: "o1",
          name: "#1042",
          status: "Draft",
          _workflow: {
            current: "Draft",
            transitions: [{ name: "submit", label: "Submit", from: "Draft", to: "Submitted" }],
          },
        },
      ],
      total: 1,
      page: 1,
      page_size: 25,
    });
    wrap(<EntityList entities={[order]} />, "/orders");
    await waitFor(() => expect(screen.getByRole("button", { name: "Submit" })).toBeInTheDocument());
    expect(screen.getByText("Actions")).toBeInTheDocument();
  });

  it("runs bulk archive through the entity runtime", async () => {
    const bulk = vi.spyOn(api, "bulk").mockResolvedValue({
      action: "archive",
      succeeded: 1,
      failed: 0,
      results: [{ id: "c1", ok: true }],
    });
    vi.spyOn(api, "list").mockResolvedValue({
      items: [{ id: "c1", name: "Ada" }],
      total: 1,
      page: 1,
      page_size: 25,
    });
    const customer: UiEntity = {
      ...order,
      entity: "Customer",
      label: "Customer",
      label_plural: "Customers",
      slug: "customers",
      workflow: undefined,
      capabilities: { ...order.capabilities, archive: true, bulk: true, assignment: false },
      permissions: { list: true, create: true, read: true, update: true, delete: true, export: true },
    };
    wrap(<EntityList entities={[customer]} />, "/customers");
    await userEvent.click(await screen.findByLabelText("Select row"));
    expect(screen.getByText("1 customer selected")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Archive selected" }));
    expect(bulk).not.toHaveBeenCalled();
    expect(screen.getByRole("dialog", { name: "Archive 1 customer?" })).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(bulk).not.toHaveBeenCalled();
    await userEvent.click(screen.getByRole("button", { name: "Archive selected" }));
    await userEvent.click(screen.getByRole("button", { name: "Archive" }));
    expect(bulk).toHaveBeenCalledWith("customers", {
      action: "archive",
      ids: ["c1"],
      fields: {},
    });
  });

  it("assigns selected rows from a user search dialog", async () => {
    const bulk = vi.spyOn(api, "bulk").mockResolvedValue({
      action: "assign",
      succeeded: 1,
      failed: 0,
      results: [{ id: "c1", ok: true }],
    });
    vi.spyOn(api, "list").mockImplementation(async (slug) => {
      if (slug === "users") {
        return {
          items: [{ id: "u1", name: "Ada Lovelace", email: "ada@example.com" }],
          total: 1,
          page: 1,
          page_size: 20,
        };
      }
      return { items: [{ id: "c1", name: "Ada" }], total: 1, page: 1, page_size: 25 };
    });
    const customer: UiEntity = {
      ...order,
      entity: "Customer",
      label: "Customer",
      label_plural: "Customers",
      slug: "customers",
      workflow: undefined,
      capabilities: { ...order.capabilities, archive: false, bulk: true, assignment: true },
      permissions: { list: true, create: true, read: true, update: true, delete: true, export: true },
    };
    wrap(<EntityList entities={[customer]} />, "/customers");
    await userEvent.click(await screen.findByLabelText("Select row"));
    await userEvent.click(screen.getByRole("button", { name: "Assign…" }));
    expect(await screen.findByRole("dialog", { name: "Assign 1 customer" })).toBeInTheDocument();
    await userEvent.click(await screen.findByRole("option", { name: /Ada Lovelace/ }));
    await userEvent.click(screen.getByRole("button", { name: "Assign" }));
    expect(bulk).toHaveBeenCalledWith("customers", {
      action: "assign",
      ids: ["c1"],
      fields: { assigned_to: "u1" },
    });
  });

  it("renders an admin audit table", async () => {
    vi.spyOn(api, "audit").mockResolvedValue({
      items: [
        {
          id: "1",
          actor: "Ahmed",
          entity: "Order",
          entity_id: "1042-aaaa",
          action: "update",
          created_at: new Date().toISOString(),
          changes: { status: { old: "Lead", new: "Qualified" } },
        },
      ],
    });
    wrap(<AuditLog />, "/settings/audit");
    expect(await screen.findByText("Ahmed")).toBeInTheDocument();
    expect(screen.getByText(/status: Lead → Qualified/)).toBeInTheDocument();
  });

  it("hides audit history from unauthorized users", async () => {
    vi.spyOn(api, "audit").mockRejectedValue(new ApiError("audit log requires Admin", 403));
    wrap(<AuditLog />, "/settings/audit");
    expect(await screen.findByText("Not authorized")).toBeInTheDocument();
  });

  it("shows notification relative time in the bell", async () => {
    vi.spyOn(api, "notifications").mockResolvedValue({
      unread: 1,
      items: [
        {
          id: "n1",
          title: "Order #1042 is ready",
          created_at: new Date().toISOString(),
        },
      ],
    });
    render(
      <TenantThemeContext.Provider value={{ timezone: "UTC", locale: "en-US", currency: "USD" }}>
        <MemoryRouter>
          <NotificationBell entities={[order]} />
        </MemoryRouter>
      </TenantThemeContext.Provider>,
    );
    await userEvent.click(screen.getByRole("button", { name: "Notifications" }));
    expect(await screen.findByText("Order #1042 is ready")).toBeInTheDocument();
    expect(screen.getByText(/second|now|minute/i)).toBeInTheDocument();
  });
});
