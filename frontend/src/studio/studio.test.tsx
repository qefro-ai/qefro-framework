import { MemoryRouter, Route, Routes } from "react-router-dom";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import Overview from "./pages/Overview";
import FieldEditor from "./editors/FieldEditor";
import FormPreview from "./preview/FormPreview";
import SourceView from "./components/SourceView";
import Permissions from "./pages/Permissions";
import Workflows from "./pages/Workflows";
import CommandPalette from "./components/CommandPalette";
import AutomationsStudio from "./pages/AutomationsStudio";
import type { UiEntity } from "../api";
import { TenantThemeContext } from "../metadata/context";
import "../widgets";

const overview = {
  installed_apps: 4,
  entities: 37,
  workflows: 8,
  reports: 19,
  dashboards: 7,
  pages: 2,
  apps: [
    { name: "restaurant", label: "Restaurant", version: "1.2.0" },
    { name: "crm", label: "CRM", version: "1.1.0" },
  ],
  warnings: [{ kind: "disabled_app", app: "inventory" }],
  recent_changes: [],
};

beforeEach(() => {
  vi.stubGlobal(
    "fetch",
    vi.fn(async (input: RequestInfo) => {
      const url = String(input);
      const json = (body: unknown) =>
        Promise.resolve({
          ok: true,
          status: 200,
          json: async () => body,
        });
      if (url.includes("/studio/overview")) return json(overview);
      if (url.includes("/studio/entities/Order") || url.includes("/studio/entities/Reservation")) {
        return json({
          entity: {
            name: "Order",
            fields: [
              { name: "customer", type: "relation", label: "Customer", relation: { target_entity: "Customer" } },
              { name: "subtotal", type: "decimal", label: "Subtotal", computed: true, formula: "SUM(items.amount)", ui: { widget: "currency" } },
            ],
          },
          json: '{"name":"Order"}',
          yaml: "name: Order\n",
        });
      }
      if (url.includes("/studio/entities")) {
        return json({
          entities: [
            { name: "Order", label: "Order", module: "restaurant", workflow: "order" },
            { name: "Reservation", label: "Reservation", module: "restaurant", workflow: "reservation" },
          ],
        });
      }
      if (url.includes("/studio/workflows")) {
        return json({
          workflow: {
            name: "order",
            entity: "Order",
            initial: "Draft",
            states: [{ name: "Draft" }, { name: "Confirmed" }],
            transitions: [{ name: "confirm", from: "Draft", to: "Confirmed", label: "Confirm", allowed_roles: ["Staff"] }],
          },
          json: "{}",
          yaml: "name: order\n",
        });
      }
      if (url.includes("/studio/permissions")) {
        return json({
          grants: [{ role: "Staff", entity: "Order", actions: ["create", "read", "list"] }],
        });
      }
      if (url.includes("/studio/operations")) {
        return json({ operations: [{ name: "approve_order", label: "Approve", roles: ["Manager"], source_managed: true }] });
      }
      if (url.includes("/studio/search")) {
        return json({ results: [{ kind: "entity", name: "Order", label: "Order" }] });
      }
      if (url.includes("/studio/validate") || url.includes("/studio/publish")) {
        return json({ ok: true, impact: "safe", migration_required: false, warnings: [], diff: ["~ status.label"] });
      }
      if (url.includes("/studio/automations/") && url.includes("/runs")) {
        return json({ runs: [{ execution_id: "1", automation_id: "order_confirmed_followup", status: "completed", steps: [] }] });
      }
      if (url.includes("/studio/automations/") && url.includes("/preview")) {
        return json({ automation: "order_confirmed_followup", dry_run: true, would_execute: [{ kind: "notify" }], side_effects: false });
      }
      if (url.includes("/studio/automations/") && !url.endsWith("/automations")) {
        return json({
          automation: {
            name: "order_confirmed_followup",
            enabled: true,
            description: "Follow up",
            version: 1,
            status: "published",
            trigger: { type: "event", event: "order.confirmed" },
            steps: [
              { kind: "send_communication", action: { send_communication: { template: "order_confirmed" } } },
              { kind: "wait", wait: "30m", label: "wait 30m" },
              {
                kind: "condition",
                condition: { field: "status", equals: "Preparing" },
                then: [{ kind: "notify", action: { notify: { role: "Manager" } } }],
                else: [],
              },
            ],
          },
          json: "{}",
          yaml: "name: order_confirmed_followup\n",
        });
      }
      if (url.includes("/studio/automations")) {
        return json({
          automations: [{ name: "order_confirmed_followup", status: "published", version: 1 }],
        });
      }
      return json({});
    }),
  );
});

afterEach(() => {
  vi.unstubAllGlobals();
});

const orderUi: UiEntity = {
  entity: "Order",
  label: "Order",
  label_plural: "Orders",
  slug: "orders",
  searchable: true,
  fields: [
    {
      name: "customer",
      type: "relation",
      label: "Customer",
      required: true,
      list: true,
      form: true,
      filter: true,
      searchable: true,
      readonly: false,
      widget: "relation",
    },
  ],
};

describe("Studio UI", () => {
  it("renders overview stats from the API", async () => {
    render(
      <MemoryRouter>
        <Overview />
      </MemoryRouter>,
    );
    expect(await screen.findByText("Qefro Studio")).toBeInTheDocument();
    expect(await screen.findByText("37")).toBeInTheDocument();
    expect(screen.getByText("Restaurant")).toBeInTheDocument();
    expect(screen.getByText(/disabled app/i)).toBeInTheDocument();
  });

  it("shows currency options only for the currency widget", async () => {
    render(
      <FieldEditor
        entity="Order"
        fields={[
          {
            name: "subtotal",
            type: "decimal",
            label: "Subtotal",
            ui: { widget: "currency", widget_options: { currency: "INR", precision: 2 } },
          },
        ]}
        canEdit
        canPublish
        onSaved={async () => undefined}
      />,
    );
    expect(screen.getByDisplayValue("currency")).toBeInTheDocument();
    expect(screen.getByText("Currency")).toBeInTheDocument();
    expect(screen.getByDisplayValue("INR")).toBeInTheDocument();
  });

  it("previews the generic form renderer", () => {
    render(
      <MemoryRouter>
        <TenantThemeContext.Provider value={{ timezone: "UTC", locale: "en-US", currency: "USD" }}>
          <FormPreview entity={orderUi} />
        </TenantThemeContext.Provider>
      </MemoryRouter>,
    );
    expect(screen.getByText("Order preview")).toBeInTheDocument();
    expect(screen.getByText(/Customer/)).toBeInTheDocument();
  });

  it("switches JSON and YAML source views", () => {
    render(<SourceView jsonText='{"name":"Order"}' yamlText="name: Order" />);
    expect(screen.getByText('{"name":"Order"}')).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "YAML" }));
    expect(screen.getByText("name: Order")).toBeInTheDocument();
  });

  it("renders a permission matrix and operation list", async () => {
    render(
      <MemoryRouter initialEntries={["/Order"]}>
        <Routes>
          <Route path="/:entity" element={<Permissions caps={["studio.view", "studio.manage_permissions"]} />} />
        </Routes>
      </MemoryRouter>,
    );
    expect(await screen.findByText("Order permissions")).toBeInTheDocument();
        expect(await screen.findByText(/Approve/)).toBeInTheDocument();
    expect(screen.getByText(/Source-managed/)).toBeInTheDocument();
  });

  it("exposes permission level and allow-on-submit on fields", () => {
    render(
      <FieldEditor
        entity="Invoice"
        fields={[
          {
            name: "delivery_note",
            type: "text",
            label: "Delivery Note",
            permission_level: 0,
            allow_on_submit: true,
            ui: { widget: "textarea" },
          },
        ]}
        canEdit
        canPublish
        onSaved={async () => undefined}
      />,
    );
    expect(screen.getByText(/Permission level/)).toBeInTheDocument();
    expect(screen.getByLabelText(/Allow on submit/)).toBeInTheDocument();
  });

  it("renders workflow states and transitions", async () => {
    render(
      <MemoryRouter initialEntries={["/Order"]}>
        <Routes>
          <Route path="/:entity" element={<Workflows caps={["studio.view", "studio.manage_workflows"]} />} />
        </Routes>
      </MemoryRouter>,
    );
    expect(await screen.findByText("Order workflow")).toBeInTheDocument();
    expect(screen.getByText(/Confirm → Confirmed/)).toBeInTheDocument();
  });

  it("opens the command palette with ctrl/k", async () => {
    render(
      <MemoryRouter>
        <CommandPalette caps={["studio.view"]} />
      </MemoryRouter>,
    );
    fireEvent.keyDown(window, { key: "k", ctrlKey: true });
    expect(await screen.findByLabelText("Studio command palette")).toBeInTheDocument();
    fireEvent.change(screen.getByPlaceholderText(/Search metadata/), { target: { value: "Order" } });
    await waitFor(() => expect(screen.getByText(/entity: Order/i)).toBeInTheDocument());
  });

  it("edits automation nodes and shows validation", async () => {
    render(
      <MemoryRouter initialEntries={["/order_confirmed_followup"]}>
        <Routes>
          <Route path="/:name" element={<AutomationsStudio caps={["studio.view", "studio.publish"]} />} />
        </Routes>
      </MemoryRouter>,
    );
    expect(await screen.findByDisplayValue("order.confirmed")).toBeInTheDocument();
    expect(screen.getByDisplayValue("Follow up")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Add wait" }));
    expect(screen.getAllByText("Wait").length).toBeGreaterThan(1);
    fireEvent.click(screen.getByRole("button", { name: "Publish" }));
    expect(screen.getByRole("button", { name: "Disable" })).toBeInTheDocument();
    expect(screen.getByText(/Automation Runs|No automation runs|completed/i)).toBeTruthy();
  });
});
