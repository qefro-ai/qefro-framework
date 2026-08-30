import { t, entityCount } from "./i18n";

describe("i18n chrome", () => {
  it("substitutes variables and allows terminology override", () => {
    expect(t("bulk.selected", { count: "3 customers" })).toBe("3 customers selected");
    expect(t("bulk.archive")).toBe("Archive selected");
    expect(t("bulk.delete", undefined, { "bulk.delete": "Remove" })).toBe("Remove");
  });

  it("uses singular and plural entity nouns", () => {
    expect(entityCount(1, "Customer", "Customers")).toBe("1 customer");
    expect(entityCount(3, "Customer", "Customers")).toBe("3 customers");
    expect(t("bulk.archiveTitle", { count: entityCount(1, "Customer", "Customers") })).toBe("Archive 1 customer?");
  });
});
