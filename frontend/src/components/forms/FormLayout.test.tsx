import { render, screen, waitFor } from "@testing-library/react";
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

  it("renders metadata columns and tab error indicators", async () => {
    const layoutFields: UiField[] = [
      { ...fields[0], name: "name", label: "Name", width: "half" },
      { ...fields[0], name: "email", label: "Email", required: false, section: undefined },
      {
        ...fields[0],
        name: "status",
        label: "Status",
        required: false,
        readonly: false,
        readonly_when: { field: "status", equals: "Completed" },
        section: undefined,
      },
    ];
    render(
      <FormLayout
        fields={layoutFields}
        values={{ name: "", email: "", status: "Completed" }}
        entities={[]}
        fieldErrors={{ email: "Invalid email" }}
        layout={[
          {
            title: "Customer Information",
            tab: "Details",
            columns: [{ fields: ["name"] }, { fields: ["email"] }],
          },
          { title: "Advanced", tab: "Advanced", fields: ["status"] },
        ]}
        onChange={() => undefined}
      />,
    );
    expect(screen.getByRole("tab", { name: /Details/ })).toHaveClass("has-error");
    expect(screen.getByLabelText(/Name/)).toBeInTheDocument();
    expect(screen.getByLabelText(/Email/)).toBeInTheDocument();
    await screen.getByRole("tab", { name: "Advanced" }).click();
    expect(screen.getByLabelText(/Status/)).toBeDisabled();
  });

  it("reveals a collapsed section when focusing an invalid field", async () => {
    render(
      <FormLayout
        fields={fields}
        values={{ name: "" }}
        entities={[]}
        fieldErrors={{ name: "Name is required" }}
        layout={[{ title: "Customer", collapsed: true, fields: ["name"] }]}
        focusField="name"
        onChange={() => undefined}
      />,
    );
    expect(await screen.findByLabelText(/Name/)).toBeInTheDocument();
    await waitFor(() => expect(document.activeElement).toHaveAttribute("id", "field-name"));
  });

  it("renders a two-column form grid from layout metadata", () => {
    const { container } = render(
      <FormLayout
        fields={[
          { ...fields[0], name: "name" },
          { ...fields[0], name: "email", label: "Email", required: false },
        ]}
        values={{}}
        entities={[]}
        fieldErrors={{}}
        layout={[
          {
            title: "Customer Information",
            columns: [{ fields: ["name"] }, { fields: ["email"] }],
          },
        ]}
        onChange={() => undefined}
      />,
    );
    expect(container.querySelector(".form-columns")).toBeTruthy();
    expect(container.querySelectorAll(".form-column")).toHaveLength(2);
  });
});
