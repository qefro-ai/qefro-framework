import { primaryNavEntities, settingsEntities } from "./navigation";
import type { UiEntity } from "./types";

function entity(over: Partial<UiEntity> & { entity: string; slug: string }): UiEntity {
  return {
    label: over.label ?? over.entity,
    label_plural: over.label_plural ?? `${over.entity}s`,
    searchable: true,
    fields: [],
    standalone: true,
    ...over,
  };
}

const reservations = entity({ entity: "Reservation", slug: "reservations", label_plural: "Reservations" });
const tables = entity({ entity: "DiningTable", slug: "tables", label: "Table", label_plural: "Tables" });
const restaurants = entity({
  entity: "Restaurant",
  slug: "restaurants",
  label_plural: "Restaurants",
  description: "Locations",
});
const settings = entity({
  entity: "RestaurantSettings",
  slug: "restaurant-settings",
  label: "Restaurant Settings",
  label_plural: "Restaurant Settings",
  singleton: true,
});
const leads = entity({ entity: "Lead", slug: "leads", label_plural: "Leads", module: "crm" });
const lines = entity({
  entity: "OrderItem",
  slug: "order-items",
  standalone: false,
  child_of: "Order",
});

describe("primaryNavEntities", () => {
  it("orders listed slugs and hides the rest of that app", () => {
    const nav = primaryNavEntities(
      [restaurants, reservations, tables, settings, leads, lines],
      ["reservations", "tables"],
      ["restaurants", "restaurant-settings"],
    );
    expect(nav.map((e) => e.slug)).toEqual(["reservations", "tables", "leads"]);
  });

  it("keeps an explicit nav item even if it is also hidden", () => {
    const nav = primaryNavEntities(
      [reservations, restaurants],
      ["restaurants"],
      ["restaurants", "reservations"],
    );
    expect(nav.map((e) => e.slug)).toEqual(["restaurants"]);
  });
});

describe("settingsEntities", () => {
  it("lists hidden setup entities and singletons", () => {
    const setup = settingsEntities(
      [restaurants, reservations, tables, settings, leads, lines],
      ["reservations", "tables"],
      ["restaurants", "restaurant-settings"],
    );
    expect(setup.map((e) => e.slug)).toEqual(["restaurant-settings", "restaurants"]);
  });

  it("keeps People and Users in Settings, not ops nav", () => {
    const people = entity({ entity: "Person", slug: "people", label_plural: "People" });
    const users = entity({ entity: "User", slug: "users", label_plural: "Users" });
    const nav = primaryNavEntities(
      [reservations, tables, people, users],
      ["reservations", "tables"],
      ["people", "users"],
    );
    expect(nav.map((e) => e.slug)).toEqual(["reservations", "tables"]);
    const setup = settingsEntities(
      [reservations, tables, people, users],
      ["reservations", "tables"],
      ["people", "users"],
    );
    expect(setup.map((e) => e.slug)).toEqual(["people", "users"]);
  });
});
