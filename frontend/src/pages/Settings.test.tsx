import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import Settings from "./Settings";
import type { TenantConfig, UiEntity } from "../api";

const config: TenantConfig = {
  branding: { company_name: "Demo Kitchen" },
  ui_config: { navigation: ["reservations"], hidden_entities: ["restaurants"] },
  enabled_apps: ["restaurant"],
};

const restaurant: UiEntity = {
  entity: "Restaurant",
  label: "Restaurant",
  label_plural: "Restaurants",
  slug: "restaurants",
  searchable: true,
  standalone: true,
  description: "Locations, branding, and contact details",
  fields: [],
};

describe("Settings setup", () => {
  it("links hidden configuration entities", () => {
    render(
      <MemoryRouter>
        <Settings
          config={config}
          entities={[restaurant]}
          navSlugs={["reservations"]}
          hiddenEntities={["restaurants"]}
          onSaved={() => undefined}
        />
      </MemoryRouter>,
    );
    expect(screen.getByRole("link", { name: /Restaurants/ })).toHaveAttribute("href", "/restaurants");
    expect(screen.getByText("Workspace settings")).toBeInTheDocument();
  });
});
