import { t } from "./i18n";

describe("i18n chrome", () => {
  it("substitutes variables and allows terminology override", () => {
    expect(t("bulk.selected", { n: 3 })).toBe("3 selected");
    expect(t("bulk.archive")).toBe("Archive selected");
    expect(t("bulk.delete", undefined, { "bulk.delete": "Remove" })).toBe("Remove");
  });
});
