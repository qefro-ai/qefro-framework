import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import CardView from "./CardView";
import type { UiEntity, UiField } from "../../metadata/types";

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

const guest: UiEntity = {
  entity: "Guest",
  label: "Guest",
  label_plural: "Guests",
  slug: "guests",
  searchable: true,
  display_field: "name",
  views: { card: { title: "name", subtitle: "email", fields: ["status"] } },
  fields: [
    field({ name: "name" }),
    field({ name: "email" }),
    field({ name: "status", widget: "status" }),
  ],
};

describe("CardView", () => {
  it("renders title, subtitle, fields and navigates to the record", () => {
    render(
      <MemoryRouter>
        <CardView
          meta={guest}
          entities={[guest]}
          slug="guests"
          rows={[{ id: "g1", name: "Ada", email: "ada@example.com", status: "Active" }]}
          total={1}
          loading={false}
          onReload={() => undefined}
          onError={() => undefined}
        />
      </MemoryRouter>,
    );
    expect(screen.getByText("Ada")).toBeInTheDocument();
    expect(screen.getByText("ada@example.com")).toBeInTheDocument();
    expect(screen.getByText("Active")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Ada" })).toHaveAttribute("href", "/guests/g1");
    expect(screen.getByRole("link", { name: "View" })).toHaveAttribute("href", "/guests/g1");
  });

  it("uses a card skeleton while the first page loads", () => {
    render(
      <MemoryRouter>
        <CardView
          meta={guest}
          entities={[guest]}
          slug="guests"
          rows={[]}
          total={0}
          loading
          onReload={() => undefined}
          onError={() => undefined}
        />
      </MemoryRouter>,
    );
    expect(screen.getByText("Loading")).toBeInTheDocument();
  });

  it("hides create when permissions.create is false", () => {
    render(
      <MemoryRouter>
        <CardView
          meta={{ ...guest, permissions: { create: false } }}
          entities={[guest]}
          slug="guests"
          rows={[]}
          total={0}
          loading={false}
          onReload={() => undefined}
          onError={() => undefined}
        />
      </MemoryRouter>,
    );
    expect(screen.queryByRole("button", { name: /New Guest/ })).not.toBeInTheDocument();
  });
});
