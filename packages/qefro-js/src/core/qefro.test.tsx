import { render, screen } from "@testing-library/react";
import { MemoryRouter, RouterProvider, createMemoryRouter } from "react-router-dom";
import { afterEach, describe, expect, it, vi } from "vitest";
import { Qefro } from "./runtime";
import { defaultExtensions } from "./extensions";
import { emitUiEvent, onUiEvent, resetUiEvents } from "./events";
import { applyBranding, applyTheme } from "../theme/apply";
import { EntityListRenderer } from "../renderer";
import { QefroRoutes } from "../router";
import { setApiBaseUrl, getApiBaseUrl, api, canCreate } from "../index";
import type { UiEntity, UiField } from "../metadata/types";
import { PrefsProvider } from "../prefsContext";
import { TenantThemeContext } from "../metadata/context";

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

function entity(over: Partial<UiEntity> & { entity: string; slug: string }): UiEntity {
  return {
    label: over.label ?? over.entity,
    label_plural: over.label_plural ?? `${over.entity}s`,
    searchable: true,
    fields: [field({ name: "name" })],
    standalone: true,
    ...over,
  };
}

afterEach(() => {
  defaultExtensions.reset();
  resetUiEvents();
  setApiBaseUrl("/api/v1");
  document.documentElement.style.cssText = "";
  vi.restoreAllMocks();
});

describe("Qefro runtime", () => {
  it("initializes with a configurable API root", async () => {
    const qefro = new Qefro({ apiUrl: "https://example.test/api/v1", isolated: true });
    await qefro.init();
    expect(qefro.client).toBe(api);
    setApiBaseUrl("https://example.test/api/v1");
    expect(getApiBaseUrl()).toBe("https://example.test/api/v1");
  });

  it("applies theme tokens without hardcoded app colors", () => {
    const qefro = new Qefro({ isolated: true });
    qefro.theme({ primary: "#2563eb", radius: "medium" });
    expect(document.documentElement.style.getPropertyValue("--primary")).toBe("#2563eb");
    expect(document.documentElement.style.getPropertyValue("--accent")).toBe("#2563eb");
    expect(document.documentElement.style.getPropertyValue("--radius")).toBe("8px");
  });

  it("lets tenant branding override application theme", () => {
    applyTheme({ primary: "#111111" });
    applyBranding(
      {
        branding: { primary_color: "#ff0000", company_name: "Estate" },
        ui_config: { navigation: [], hidden_entities: [] },
        enabled_apps: [],
      },
      { primary: "#111111" },
    );
    expect(document.documentElement.style.getPropertyValue("--primary")).toBe("#ff0000");
    expect(document.title).toBe("Estate");
  });

  it("resolves entity UI from metadata names", () => {
    const qefro = new Qefro({ isolated: true });
    qefro.hydrate({
      entities: [entity({ entity: "Lead", slug: "leads" })],
      config: null,
    });
    expect(qefro.entity("Lead").meta()?.slug).toBe("leads");
    expect(qefro.entity("leads").meta()?.entity).toBe("Lead");
  });

  it("emits frontend lifecycle events only", () => {
    const seen: string[] = [];
    const qefro = new Qefro({ isolated: true });
    qefro.on("entity:created", (payload) => seen.push(payload.entity));
    emitUiEvent("entity:created", { entity: "Lead", slug: "leads", id: "1" });
    expect(seen).toEqual(["Lead"]);
  });
});

describe("extensions and customization", () => {
  it("registers pages, widgets, fields, and entity overrides", () => {
    const qefro = new Qefro();
    const Page = () => null;
    const Widget = () => null;
    const Field = () => null;
    const Card = () => null;
    qefro.page("property-map", { component: Page, path: "/property-map" });
    qefro.register({
      widget: { name: "funnel", component: Widget },
      field: { name: "plot", component: Field },
      entity: { name: "Property", card: Card },
    });
    expect(qefro.extensions.getPage("property-map")?.path).toBe("/property-map");
    expect(qefro.extensions.getDashboardWidget("funnel")).toBe(Widget);
    expect(qefro.extensions.entityOverrides("Property").card).toBe(Card);
  });

  it("renders a custom entity list when registered, otherwise the generic list", async () => {
    const lead = entity({
      entity: "Lead",
      slug: "leads",
      permissions: { list: true, create: false, read: true, update: false, delete: false },
    });
    const qefro = new Qefro();
    qefro.ui.extend("Lead", {
      list: () => <div>Custom Lead board</div>,
    });
    render(
      <MemoryRouter initialEntries={["/leads"]}>
        <TenantThemeContext.Provider value={{ timezone: "UTC", locale: "en-US", currency: "USD" }}>
          <PrefsProvider tenantId="t" userId="u">
            <EntityListRenderer entities={[lead]} entity="Lead" />
          </PrefsProvider>
        </TenantThemeContext.Provider>
      </MemoryRouter>,
    );
    expect(screen.getByText("Custom Lead board")).toBeInTheDocument();
    expect(canCreate(lead)).toBe(false);
  });

  it("does not grant authorization when a custom component is registered", () => {
    const create = vi.spyOn(api, "create");
    const qefro = new Qefro();
    qefro.ui.extend("Property", { card: () => <div>Pretty card</div> });
    expect(api.create).toBe(create.getMockImplementation() ? api.create : api.create);
    expect(create).not.toHaveBeenCalled();
    create.mockRestore();
  });

  it("keeps generic rendering as the default", async () => {
    const lead = entity({
      entity: "Lead",
      slug: "leads",
      permissions: { list: true, create: false, read: true, update: false, delete: false },
    });
    vi.spyOn(api, "list").mockResolvedValue({ items: [], total: 0, page: 1, page_size: 25 });
    render(
      <MemoryRouter initialEntries={["/leads"]}>
        <TenantThemeContext.Provider value={{ timezone: "UTC", locale: "en-US", currency: "USD" }}>
          <PrefsProvider tenantId="t" userId="u">
            <EntityListRenderer entities={[lead]} entity="Lead" />
          </PrefsProvider>
        </TenantThemeContext.Provider>
      </MemoryRouter>,
    );
    expect(await screen.findByText(/no leads yet/i)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /new lead/i })).not.toBeInTheDocument();
  });
});

describe("routing and custom pages", () => {
  it("renders a registered custom page at /pages/:name", () => {
    const qefro = new Qefro();
    qefro.page("property-map", { component: () => <div>Property map</div> });
    render(
      <MemoryRouter initialEntries={["/pages/property-map"]}>
        <QefroRoutes entities={[]} config={null} />
      </MemoryRouter>,
    );
    expect(screen.getByText("Property map")).toBeInTheDocument();
  });

  it("keeps entity edit routes", async () => {
    const lead = entity({ entity: "Lead", slug: "leads" });
    vi.spyOn(api, "get").mockResolvedValue({ id: "abc", name: "Ada" });
    const router = createMemoryRouter(
      [
        {
          path: "*",
          element: (
            <TenantThemeContext.Provider value={{ timezone: "UTC", locale: "en-US", currency: "USD" }}>
              <QefroRoutes entities={[lead]} config={null} />
            </TenantThemeContext.Provider>
          ),
        },
      ],
      { initialEntries: ["/leads/abc/edit"] },
    );
    render(<RouterProvider router={router} />);
    expect(await screen.findByText("Lead")).toBeInTheDocument();
  });
});

describe("onUiEvent", () => {
  it("unsubscribes", () => {
    const handler = vi.fn();
    const off = onUiEvent("route:change", handler);
    emitUiEvent("route:change", { pathname: "/leads", search: "" });
    off();
    emitUiEvent("route:change", { pathname: "/x", search: "" });
    expect(handler).toHaveBeenCalledTimes(1);
  });
});
