import { resolveLayout, tabHasError } from "./layout";
import type { UiField, ViewSection } from "./types";

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

describe("resolveLayout", () => {
  const fields = [
    field({ name: "name", label: "Name", section: "Contact" }),
    field({ name: "email", label: "Email", section: "Contact" }),
    field({ name: "party_type", label: "Party type" }),
    field({ name: "company_name", label: "Company", visible_when: { field: "party_type", equals: "Organization" } }),
    field({ name: "notes", label: "Notes", width: "full" }),
  ];

  it("falls back to field.section grouping without layout spec", () => {
    const layout = resolveLayout(fields, undefined, { party_type: "Person" });
    expect(layout.sections.some((s) => s.title === "Contact")).toBe(true);
    expect(layout.sections.flatMap((s) => s.columns.flatMap((c) => c.fields.map((f) => f.name)))).not.toContain(
      "company_name",
    );
  });

  it("honors columns, tabs, and section visibility", () => {
    const spec: ViewSection[] = [
      {
        title: "Customer Information",
        tab: "Details",
        columns: [
          { fields: ["name", "email"] },
          { fields: ["party_type"] },
        ],
      },
      {
        title: "Organization Details",
        tab: "Details",
        fields: ["company_name"],
        visible_when: { field: "party_type", equals: "Organization" },
      },
      { title: "Notes", tab: "Advanced", fields: ["notes"] },
    ];
    const person = resolveLayout(fields, spec, { party_type: "Person" });
    expect(person.tabs).toEqual(["Details", "Advanced"]);
    expect(person.sections.find((s) => s.title === "Organization Details")).toBeUndefined();
    const org = resolveLayout(fields, spec, { party_type: "Organization" });
    expect(org.sections.find((s) => s.title === "Organization Details")).toBeTruthy();
    const info = org.sections.find((s) => s.title === "Customer Information")!;
    expect(info.columns).toHaveLength(2);
    expect(info.columns[0].fields.map((f) => f.name)).toEqual(["name", "email"]);
  });

  it("marks tabs that contain field errors", () => {
    const spec: ViewSection[] = [
      { title: "A", tab: "Details", fields: ["name"] },
      { title: "B", tab: "Advanced", fields: ["notes"] },
    ];
    const layout = resolveLayout(fields, spec, {});
    expect(tabHasError(layout, "Advanced", { notes: "Required" })).toBe(true);
    expect(tabHasError(layout, "Details", { notes: "Required" })).toBe(false);
  });
});
