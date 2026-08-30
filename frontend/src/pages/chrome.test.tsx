import type { ReactElement } from "react";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { api } from "../sdk/client";
import EntityDetail from "./EntityDetail";
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

const note: UiEntity = {
  entity: "Note",
  label: "Note",
  label_plural: "Notes",
  slug: "notes",
  searchable: true,
  display_field: "title",
  standalone: true,
  fields: [field({ name: "title", label: "Title" })],
  links: [{ label: "Follow-ups", entity: "FollowUp", relation: "note_id" }],
  permissions: { list: true, create: false, read: true, update: false, delete: false },
};

function wrap(ui: ReactElement, path: string) {
  return render(
    <TenantThemeContext.Provider value={{ timezone: "UTC", locale: "en-US", currency: "USD" }}>
      <MemoryRouter initialEntries={[path]}>
        <BreadcrumbRecordProvider>
          <Routes>
            <Route path="/:slug" element={ui} />
            <Route path="/:slug/:id" element={ui} />
          </Routes>
        </BreadcrumbRecordProvider>
      </MemoryRouter>
    </TenantThemeContext.Provider>,
  );
}

describe("permission chrome", () => {
  beforeEach(() => {
    vi.spyOn(api, "list").mockResolvedValue({ items: [], total: 0, page: 1, page_size: 25 });
    vi.spyOn(api, "get").mockResolvedValue({
      id: "n1",
      title: "Hello",
      _permissions: { update: false, delete: false },
      _links: [{ label: "Follow-ups", entity: "FollowUp", slug: "follow-ups", relation: "note_id", total: 0 }],
    });
    vi.spyOn(api, "audit").mockResolvedValue({ items: [] });
    vi.spyOn(api, "activity").mockResolvedValue({ items: [] });
  });
  afterEach(() => vi.restoreAllMocks());

  it("hides New on the list when create is false", async () => {
    wrap(<EntityList entities={[note]} />, "/notes");
    await waitFor(() => expect(screen.getByText("Notes")).toBeInTheDocument());
    expect(screen.queryByRole("button", { name: /New Note/ })).not.toBeInTheDocument();
  });

  it("hides Edit and Delete on detail when _permissions are false", async () => {
    wrap(<EntityDetail entities={[note]} />, "/notes/n1");
    await waitFor(() => expect(screen.getByText(/Note Hello/)).toBeInTheDocument());
    expect(screen.queryByRole("button", { name: "Edit" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Delete" })).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole("tab", { name: "Related records" }));
    expect(await screen.findByRole("link", { name: "Add" })).toHaveAttribute(
      "href",
      "/follow-ups/new?note_id=n1",
    );
  });

  it("links standalone child rows and leaves embedded children as text", async () => {
    const line: UiEntity = {
      entity: "Line",
      label: "Line",
      label_plural: "Lines",
      slug: "lines",
      searchable: false,
      standalone: true,
      fields: [field({ name: "name", label: "Name" })],
    };
    const embedded: UiEntity = {
      entity: "NoteTag",
      label: "Tag",
      label_plural: "Tags",
      slug: "note-tags",
      searchable: false,
      standalone: false,
      child_of: "Note",
      fields: [field({ name: "name", label: "Name" })],
    };
    const parent: UiEntity = {
      ...note,
      fields: [
        field({ name: "title", label: "Title" }),
        field({
          name: "items",
          label: "Items",
          widget: "child_table",
          type: "child_table",
          relation_kind: "child_table",
          child_entity: "Line",
        }),
        field({
          name: "tags",
          label: "Tags",
          widget: "child_table",
          type: "child_table",
          relation_kind: "child_table",
          child_entity: "NoteTag",
        }),
      ],
    };
    vi.spyOn(api, "get").mockResolvedValue({
      id: "n1",
      title: "Hello",
      items: [{ id: "l1", name: "Widget" }],
      tags: [{ id: "t1", name: "Urgent" }],
      _permissions: { update: true, delete: true },
    });
    wrap(<EntityDetail entities={[parent, line, embedded]} />, "/notes/n1");
    await waitFor(() => expect(screen.getByText(/Note Hello/)).toBeInTheDocument());
    await userEvent.click(screen.getByRole("tab", { name: "Items" }));
    expect(screen.getByRole("link", { name: "Widget" })).toHaveAttribute("href", "/lines/l1");
    await userEvent.click(screen.getByRole("tab", { name: "Tags" }));
    expect(screen.getByText("Urgent")).toBeInTheDocument();
    expect(screen.queryByRole("link", { name: "Urgent" })).not.toBeInTheDocument();
  });

  it("shows a clear-filters empty state when search has no matches", async () => {
    wrap(<EntityList entities={[note]} />, "/notes?search=zzz");
    await waitFor(() => expect(screen.getByText(/No matching notes/i)).toBeInTheDocument());
    expect(screen.getByRole("button", { name: /Clear filters/i })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /New Note/ })).not.toBeInTheDocument();
  });
});
