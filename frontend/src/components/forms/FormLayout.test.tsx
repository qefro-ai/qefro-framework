import { render, screen } from "@testing-library/react";
import { FormLayout } from "./FormLayout";
import "../../widgets";
import type { UiField } from "../../metadata/types";

const fields: UiField[] = [
  {
    name: "name",
    type: "string",
    label: "Name",
    required: true,
    list: true,
    form: true,
    filter: false,
    searchable: true,
    readonly: false,
    widget: "text",
    section: "Customer",
  },
  {
    name: "notes",
    type: "text",
    label: "Notes",
    required: false,
    list: false,
    form: true,
    filter: false,
    searchable: false,
    readonly: false,
    widget: "textarea",
    section: "Notes",
  },
];

describe("FormLayout", () => {
  it("renders sections, required marker, and server errors", () => {
    render(
      <FormLayout
        fields={fields}
        values={{ name: "" }}
        entities={[]}
        fieldErrors={{ name: "Name is required" }}
        onChange={() => undefined}
      />,
    );
    expect(screen.getByText("Customer")).toBeInTheDocument();
    expect(screen.getByText(/Name \*/)).toBeInTheDocument();
    expect(screen.getByRole("alert")).toHaveTextContent("Name is required");
  });

  it("hides a section when visible_when does not match", () => {
    render(
      <FormLayout
        fields={fields}
        values={{ name: "Ada" }}
        entities={[]}
        fieldErrors={{}}
        sectionRules={[{ title: "Notes", visible_when: { field: "name", equals: "Biz" } }]}
        onChange={() => undefined}
      />,
    );
    expect(screen.getByText("Customer")).toBeInTheDocument();
    expect(screen.queryByText("Notes")).not.toBeInTheDocument();
  });
});
