import { availableViews, calendarStartField, cardEnabled, kanbanEnabled, listGroupField } from "./views";
import type { UiEntity, UiField } from "./types";

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

function entity(over: Partial<UiEntity> & { entity: string }): UiEntity {
  return {
    label: over.entity,
    label_plural: `${over.entity}s`,
    slug: over.entity.toLowerCase(),
    searchable: true,
    fields: [],
    ...over,
  };
}

describe("view metadata", () => {
  it("always includes list and adds kanban when the entity has a workflow", () => {
    const reservation = entity({
      entity: "Reservation",
      workflow: "reservation",
      fields: [
        field({ name: "status", type: "enum", widget: "status", enum_values: ["Pending", "Confirmed"] }),
        field({ name: "reservation_date", type: "date", widget: "date" }),
      ],
    });
    expect(availableViews(reservation)).toEqual(["list", "kanban", "calendar"]);
    expect(kanbanEnabled(reservation)).toBe(true);
    expect(calendarStartField(reservation)?.name).toBe("reservation_date");
  });

  it("does not offer kanban for a status enum without a workflow", () => {
    const table = entity({
      entity: "DiningTable",
      fields: [field({ name: "status", type: "enum", widget: "status", enum_values: ["available"] })],
    });
    expect(availableViews(table)).toEqual(["list"]);
  });

  it("honors explicit calendar.enabled false", () => {
    const lead = entity({
      entity: "Lead",
      workflow: "lead",
      fields: [
        field({ name: "status", type: "enum", widget: "status", enum_values: ["New"] }),
        field({ name: "follow_up_date", type: "date", widget: "date" }),
      ],
      views: { calendar: { enabled: false } },
    });
    expect(availableViews(lead)).toEqual(["list", "kanban"]);
  });

  it("reads list grouping from view metadata", () => {
    const stock = entity({
      entity: "StockBalance",
      views: { list: { group_by: "warehouse_id" } },
      fields: [field({ name: "warehouse_id", type: "relation" })],
    });
    expect(listGroupField(stock)).toBe("warehouse_id");
  });

  it("adds Cards only when views.card is present and enabled", () => {
    const plain = entity({ entity: "Note", fields: [field({ name: "title" })] });
    expect(availableViews(plain)).toEqual(["list"]);
    expect(cardEnabled(plain)).toBe(false);
    const cards = entity({
      entity: "Guest",
      fields: [field({ name: "name" })],
      views: { card: { title: "name" } },
    });
    expect(availableViews(cards)).toEqual(["list", "card"]);
    const disabled = entity({
      entity: "HiddenCard",
      fields: [field({ name: "name" })],
      views: { card: { enabled: false, title: "name" } },
    });
    expect(availableViews(disabled)).toEqual(["list"]);
  });

  it("adds Charts when views.chart is present", () => {
    const deal = entity({
      entity: "Deal",
      views: { chart: { type: "bar", dimension: "status", measure: { field: "amount", aggregation: "sum" } } },
      fields: [field({ name: "status", type: "enum", enum_values: ["Lead"] }), field({ name: "amount", type: "decimal" })],
    });
    expect(availableViews(deal)).toEqual(["list", "chart"]);
  });

  it("uses views.default when that view is available", () => {
    const order = entity({
      entity: "Order",
      workflow: "order",
      views: { default: "kanban", kanban: { group_by: "status" } },
      fields: [field({ name: "status", type: "enum", widget: "status", enum_values: ["Draft"] })],
    });
    expect(availableViews(order)).toContain("kanban");
  });
});
