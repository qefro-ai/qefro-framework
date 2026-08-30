import type { ReactElement } from "react";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createMemoryRouter, Outlet, RouterProvider } from "react-router-dom";
import { api, ApiError } from "../sdk/client";
import EntityForm from "./EntityForm";
import EntityList from "./EntityList";
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

const userEntity: UiEntity = {
  entity: "User",
  label: "User",
  label_plural: "Users",
  slug: "users",
  searchable: true,
  display_field: "name",
  standalone: true,
  permissions: { list: true, create: true, read: true, update: true, delete: true },
  fields: [
    field({ name: "name", label: "Name", required: true }),
    field({ name: "email", label: "Email", widget: "email", required: true }),
    field({ name: "enabled", label: "Enabled", type: "boolean", widget: "checkbox" }),
    field({ name: "roles", label: "Roles", type: "json", widget: "tags" }),
    field({
      name: "password",
      label: "Password",
      widget: "password",
      secret: true,
      list: false,
      list_visible: false,
      form_visible: true,
    }),
  ],
};

const personEntity: UiEntity = {
  entity: "Person",
  label: "Person",
  label_plural: "People",
  slug: "people",
  searchable: true,
  display_field: "name",
  standalone: true,
  permissions: { list: true, create: true, read: true, update: true, delete: true },
  fields: [
    field({ name: "name", label: "Name", required: true }),
    field({ name: "email", label: "Email", widget: "email" }),
    field({
      name: "user_id",
      label: "Login",
      type: "relation",
      widget: "relation",
      relation: "User",
      relation_kind: "many_to_one",
    }),
    field({
      name: "create_account",
      label: "Create login",
      type: "boolean",
      widget: "checkbox",
      list: false,
      form_visible: true,
    }),
    field({
      name: "password",
      label: "Password",
      widget: "password",
      secret: true,
      visible_when: { field: "create_account", equals: true },
    }),
  ],
};

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

function renderUserForm(path: string) {
  const router = createMemoryRouter(
    [
      {
        element: shell(<Outlet />),
        children: [
          { path: "/users", element: <div>User list</div> },
          { path: "/users/new", element: <EntityForm entities={[userEntity]} /> },
          { path: "/users/:id", element: <div>User detail</div> },
        ],
      },
    ],
    { initialEntries: [path] },
  );
  return render(<RouterProvider router={router} />);
}

describe("identity generic UI", () => {
  beforeEach(() => {
    vi.spyOn(api, "list").mockResolvedValue({ items: [], total: 0, page: 1, page_size: 25 });
    vi.spyOn(api, "get").mockResolvedValue({ id: "u1", name: "Ada", email: "ada@ex.com" });
  });
  afterEach(() => {
    vi.restoreAllMocks();
    sessionStorage.clear();
  });

  it("lists users without rendering a password column", async () => {
    vi.spyOn(api, "list").mockResolvedValue({
      items: [{ id: "u1", name: "Ada", email: "ada@ex.com", enabled: true, roles: ["Staff"] }],
      total: 1,
      page: 1,
      page_size: 25,
    });
    wrap(<EntityList entities={[userEntity]} />, "/users");
    await waitFor(() => expect(screen.getByText("Ada")).toBeInTheDocument());
    expect(screen.queryByText("Password")).not.toBeInTheDocument();
    expect(screen.queryByText("password_hash")).not.toBeInTheDocument();
  });

  it("shows password on the user create form", async () => {
    wrap(<EntityForm entities={[userEntity]} />, "/users/new");
    expect(await screen.findByLabelText("Name *")).toBeInTheDocument();
    expect(screen.getByLabelText("Password")).toHaveAttribute("type", "password");
    expect(screen.queryByText("password_hash")).not.toBeInTheDocument();
  });

  it("offers a person → user relation and create-account checkbox", async () => {
    wrap(<EntityForm entities={[personEntity, userEntity]} />, "/people/new");
    expect(await screen.findByLabelText("Create login")).toBeInTheDocument();
    expect(screen.queryByLabelText("Password")).not.toBeInTheDocument();
    await userEvent.click(screen.getByLabelText("Create login"));
    expect(await screen.findByLabelText("Password")).toHaveAttribute("type", "password");
    expect(screen.getByText("Login")).toBeInTheDocument();
  });

  it("lists related customers on person detail without custom CRM chrome", async () => {
    vi.spyOn(api, "get").mockResolvedValue({
      id: "p1",
      name: "Ada",
      email: "ada@ex.com",
      _related: {
        shop_customers: {
          entity: "ShopCustomer",
          slug: "shop-customers",
          label: "Shop customers",
          items: [{ id: "c1", name: "Walk-in Guest" }],
          total: 1,
        },
      },
    });
    vi.spyOn(api, "audit").mockResolvedValue({ items: [] });
    vi.spyOn(api, "activity").mockResolvedValue({ items: [] });
    const shop: UiEntity = {
      entity: "ShopCustomer",
      label: "Shop customer",
      label_plural: "Shop customers",
      slug: "shop-customers",
      searchable: true,
      display_field: "name",
      standalone: true,
      permissions: { list: true, create: true, read: true, update: true, delete: true },
      fields: [
        field({ name: "name", label: "Name" }),
        field({
          name: "person_id",
          label: "Person",
          type: "relation",
          widget: "relation",
          relation: "Person",
          relation_kind: "many_to_one",
        }),
      ],
    };
    wrap(<EntityDetail entities={[personEntity, shop]} />, "/people/p1");
    await waitFor(() => expect(screen.getByText(/Person Ada/)).toBeInTheDocument());
    await userEvent.click(screen.getByRole("tab", { name: "Related records" }));
    expect(screen.getByText("Shop customers")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Walk-in Guest" })).toHaveAttribute(
      "href",
      "/shop-customers/c1",
    );
  });

  it("opens linked person and user from a customer detail", async () => {
    vi.spyOn(api, "get").mockResolvedValue({
      id: "c1",
      name: "Legacy name",
      email: "legacy@ex.com",
      person_id: "p1",
      _expanded: {
        person_id: {
          id: "p1",
          label: "Ada Lovelace",
          slug: "people",
          entity: "Person",
          _expanded: {
            user_id: {
              id: "u1",
              label: "Ada",
              slug: "users",
              entity: "User",
              enabled: true,
            },
          },
        },
      },
    });
    vi.spyOn(api, "audit").mockResolvedValue({ items: [] });
    vi.spyOn(api, "activity").mockResolvedValue({ items: [] });
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
        field({ name: "email", label: "Email" }),
        field({
          name: "person_id",
          label: "Person",
          type: "relation",
          widget: "relation",
          relation: "Person",
          relation_kind: "many_to_one",
        }),
      ],
    };
    wrap(<EntityDetail entities={[customer, personEntity, userEntity]} />, "/customers/c1");
    await waitFor(() => expect(screen.getByText(/Customer Legacy name/)).toBeInTheDocument());
    expect(screen.getByRole("link", { name: "Ada Lovelace" })).toHaveAttribute("href", "/people/p1");
    expect(screen.getByRole("link", { name: "Ada" })).toHaveAttribute("href", "/users/u1");
    expect(screen.getByText("(enabled)")).toBeInTheDocument();
    expect(screen.getByText("Legacy name")).toBeInTheDocument();
  });

  it("prefills relationship query params on create", async () => {
    renderUserForm("/users/new?email=ada@ex.com");
    expect(await screen.findByLabelText("Email *")).toHaveValue("ada@ex.com");
  });

  it("warns on unsaved navigation and stays when requested", async () => {
    renderUserForm("/users/new");
    const name = await screen.findByLabelText("Name *");
    await userEvent.type(name, "Ada");
    await userEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(await screen.findByText("Unsaved changes")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Stay" }));
    expect(screen.getByLabelText("Name *")).toHaveValue("Ada");
    expect(screen.queryByText("User list")).not.toBeInTheDocument();
  });

  it("does not interrupt navigation when the form is unchanged", async () => {
    renderUserForm("/users/new");
    expect(await screen.findByLabelText("Name *")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(await screen.findByText("User list")).toBeInTheDocument();
    expect(screen.queryByText("Unsaved changes")).not.toBeInTheDocument();
  });

  it("summarizes server field errors and focuses the field", async () => {
    vi.spyOn(api, "create").mockRejectedValue(
      new ApiError("Invalid email", 400, [{ field: "email", message: "Invalid email" }]),
    );
    renderUserForm("/users/new");
    await userEvent.type(await screen.findByLabelText("Name *"), "Ada");
    await userEvent.type(screen.getByLabelText("Email *"), "ada@ex.com");
    await userEvent.click(screen.getByRole("button", { name: "Create" }));
    expect(await screen.findByRole("button", { name: "1 error" })).toBeInTheDocument();
    expect(screen.getByText("Email: Invalid email")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Email: Invalid email" }));
    await waitFor(() => expect(document.activeElement).toHaveAttribute("id", "field-email"));
  });
});
