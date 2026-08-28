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

  it("shows password only when create_account is checked", () => {
    const identityFields: UiField[] = [
      {
        name: "create_account",
        type: "boolean",
        label: "Create login",
        required: false,
        list: false,
        form: true,
        filter: false,
        searchable: false,
        readonly: false,
        widget: "checkbox",
      },
      {
        name: "password",
        type: "string",
        label: "Password",
        required: false,
        list: false,
        form: true,
        filter: false,
        searchable: false,
        readonly: false,
        widget: "password",
        visible_when: { field: "create_account", equals: true },
      },
    ];
    const { rerender } = render(
      <FormLayout
        fields={identityFields}
        values={{ create_account: false }}
        entities={[]}
        fieldErrors={{}}
        onChange={() => undefined}
      />,
    );
    expect(screen.queryByLabelText("Password")).not.toBeInTheDocument();
    rerender(
      <FormLayout
        fields={identityFields}
        values={{ create_account: true }}
        entities={[]}
        fieldErrors={{}}
        onChange={() => undefined}
      />,
    );
    expect(screen.getByLabelText("Password")).toHaveAttribute("type", "password");
  });
});
