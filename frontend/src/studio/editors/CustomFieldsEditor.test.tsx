import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import CustomFieldsEditor from "./CustomFieldsEditor";
import "../../widgets";
import type { UiEntity } from "../../api";

const ui = {
  schema_version: "1",
  entity: "Customer",
  label: "Customer",
  label_plural: "Customers",
  slug: "customers",
  fields: [
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
    },
  ],
} as unknown as UiEntity;

describe("CustomFieldsEditor", () => {
  it("opens the add custom field form and shows a real form preview", async () => {
    const user = userEvent.setup();
    render(
      <CustomFieldsEditor
        entity="Customer"
        fields={[]}
        ui={ui}
        canEdit
        canPublish
        onSaved={async () => {}}
      />,
    );
    await user.click(screen.getByRole("button", { name: "+ Add custom field" }));
    expect(screen.getByLabelText("Field name")).toBeInTheDocument();
    expect(screen.getByLabelText("Type")).toBeInTheDocument();
    await user.type(screen.getByLabelText("Field name"), "loyalty_tier");
    expect(screen.getByText("Customer preview")).toBeInTheDocument();
  });
});
