import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { TenantThemeContext } from "../../metadata/context";
import type { UiField } from "../../metadata/types";
import { FieldValue } from "./FieldValue";

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

function wrap(ui: React.ReactElement) {
  return render(
    <TenantThemeContext.Provider value={{ timezone: "UTC", locale: "en-US", currency: "USD" }}>
      <MemoryRouter>{ui}</MemoryRouter>
    </TenantThemeContext.Provider>,
  );
}

describe("FieldValue", () => {
  it("renders status badges from field metadata", () => {
    wrap(
      <FieldValue
        row={{ status: "Pending" }}
        field={field({ name: "status", widget: "status", widget_options: { indicators: { Pending: "warning" } } })}
      />,
    );
    expect(screen.getByText("Pending").className).toMatch(/warning/);
  });

  it("links expanded relations when asked", () => {
    wrap(
      <FieldValue
        row={{
          customer_id: "c1",
          _expanded: { customer_id: { id: "c1", label: "Ada", slug: "customers" } },
        }}
        field={field({ name: "customer_id", widget: "relation", relation: "Customer" })}
        linkRelations
      />,
    );
    expect(screen.getByRole("link", { name: "Ada" })).toHaveAttribute("href", "/customers/c1");
  });

  it("shows a nested login path from person expansion", () => {
    wrap(
      <FieldValue
        row={{
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
                  label: "ada@ex.com",
                  slug: "users",
                  entity: "User",
                  enabled: false,
                },
              },
            },
          },
        }}
        field={field({ name: "person_id", widget: "relation", relation: "Person" })}
        linkRelations
      />,
    );
    expect(screen.getByRole("link", { name: "Ada Lovelace" })).toHaveAttribute("href", "/people/p1");
    expect(screen.getByRole("link", { name: "ada@ex.com" })).toHaveAttribute("href", "/users/u1");
    expect(screen.getByText("(disabled)")).toBeInTheDocument();
  });

  it("formats currency with the tenant currency", () => {
    wrap(
      <FieldValue row={{ total: 12.5 }} field={field({ name: "total", widget: "currency", type: "decimal" })} />,
    );
    expect(screen.getByText("$12.50")).toBeInTheDocument();
  });
});
