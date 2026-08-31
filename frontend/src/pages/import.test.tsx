import type { ReactElement } from "react";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { api } from "../sdk/client";
import EntityList from "./EntityList";
import { BreadcrumbRecordProvider } from "../components/shell/breadcrumbContext";
import { TenantThemeContext } from "../metadata/context";
import type { UiEntity, UiField } from "../metadata/types";

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

const customer: UiEntity = {
  entity: "Customer",
  label: "Customer",
  label_plural: "Customers",
  slug: "customers",
  searchable: true,
  display_field: "name",
  standalone: true,
  capabilities: { import: true, export: true },
  permissions: { list: true, create: true, read: true, update: true, delete: true },
  fields: [
    field({ name: "name", label: "Name", required: true }),
    field({ name: "email", label: "Email", widget: "email", required: true }),
    field({ name: "phone", label: "Phone" }),
  ],
};

function wrap(ui: ReactElement, path: string) {
  return render(
    <TenantThemeContext.Provider value={{ timezone: "UTC", locale: "en-US", currency: "USD" }}>
      <MemoryRouter initialEntries={[path]}>
        <BreadcrumbRecordProvider>
          <Routes>
            <Route path="/:slug" element={ui} />
          </Routes>
        </BreadcrumbRecordProvider>
      </MemoryRouter>
    </TenantThemeContext.Provider>,
  );
}

describe("entity import workflow", () => {
  beforeEach(() => {
    vi.spyOn(api, "list").mockResolvedValue({ items: [], total: 0, page: 1, page_size: 25 });
    vi.spyOn(api, "savedFilters").mockResolvedValue({ items: [] });
    vi.spyOn(api, "importJobs").mockResolvedValue({ items: [] });
    vi.spyOn(api, "importPreview").mockResolvedValue({
      rows: 2,
      valid: 1,
      invalid: 1,
      warnings: 1,
      columns: ["Name", "Email", "Notes"],
      mapping: [
        { column: "Name", field: "name" },
        { column: "Email", field: "email" },
        { column: "Notes", field: null },
      ],
      fields: [
        { name: "name", label: "Name", required: true, unique: false },
        { name: "email", label: "Email", required: true, unique: true },
      ],
      ignored: ["Notes"],
      match_fields: ["email"],
      errors: [{ row: 3, field: "email", message: "invalid email" }],
      sample: [{ name: "Ada", email: "ada@ex.com" }],
    });
    vi.spyOn(api, "importRun").mockResolvedValue({
      imported: 1,
      created: 1,
      updated: 0,
      skipped: 0,
      failed: 1,
      warnings: 0,
      dry_run: false,
      async_job: false,
      status: "completed_with_errors",
      errors: [{ row: 3, field: "email", message: "invalid email" }],
    });
  });
  afterEach(() => vi.restoreAllMocks());

  it("maps, previews, validates, and imports from the entity list", async () => {
    wrap(<EntityList entities={[customer]} />, "/customers");
    await waitFor(() => expect(screen.getByRole("heading", { name: "Customers" })).toBeInTheDocument());
    await userEvent.click(screen.getByRole("button", { name: "More" }));
    await userEvent.click(screen.getByRole("menuitem", { name: "Import" }));
    expect(screen.getByRole("heading", { name: "Import Customers" })).toBeInTheDocument();
    await userEvent.type(
      screen.getByPlaceholderText(/name,email/i),
      "Name,Email\nAda,ada@ex.com\nBad,not-an-email",
    );
    await userEvent.click(screen.getByRole("button", { name: "Detect columns" }));
    await waitFor(() => expect(screen.getByRole("columnheader", { name: "CSV column" })).toBeInTheDocument());
    expect(screen.getByRole("columnheader", { name: "Qefro field" })).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Preview" }));
    await waitFor(() => expect(screen.getByText(/Rows detected/)).toBeInTheDocument());
    expect(screen.getByText(/1 errors/)).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Validate import" }));
    await waitFor(() => expect(screen.getByText(/Nothing imported/)).toBeInTheDocument());
    await userEvent.click(screen.getByRole("button", { name: "Import" }));
    await waitFor(() => expect(screen.getByText("Import complete")).toBeInTheDocument());
    expect(api.importRun).toHaveBeenCalled();
  });
});
