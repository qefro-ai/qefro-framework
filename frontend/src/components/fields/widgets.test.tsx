import type { ReactElement } from "react";
import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { TenantThemeContext } from "../../metadata/context";
import type { UiField } from "../../metadata/types";
import {
  Checkbox,
  ColorPicker,
  CurrencyInput,
  DatePicker,
  DateTimePicker,
  EmailInput,
  FileUpload,
  ImageUpload,
  JsonEditor,
  NumberInput,
  PercentageInput,
  PhoneInput,
  Radio,
  RelationPicker,
  RichText,
  Select,
  Switch,
  TagsInput,
  Textarea,
  TextInput,
  TimePicker,
  UrlInput,
  MultiSelect,
} from "./widgets";

const field = (over: Partial<UiField> & Pick<UiField, "name" | "widget">): UiField => ({
  type: "string",
  label: over.label ?? over.name,
  required: false,
  list: true,
  form: true,
  filter: false,
  searchable: false,
  readonly: false,
  ...over,
});

function wrap(ui: ReactElement) {
  return render(
    <TenantThemeContext.Provider value={{ timezone: "Asia/Kolkata", locale: "en-IN", currency: "INR" }}>
      {ui}
    </TenantThemeContext.Provider>,
  );
}

describe("widgets", () => {
  it("text", async () => {
    const onChange = vi.fn();
    wrap(<TextInput field={field({ name: "name", widget: "text" })} value="" onChange={onChange} entities={[]} />);
    await userEvent.type(screen.getByRole("textbox"), "Ada");
    expect(onChange).toHaveBeenCalled();
  });

  it("textarea", async () => {
    const onChange = vi.fn();
    wrap(<Textarea field={field({ name: "bio", widget: "textarea" })} value="" onChange={onChange} entities={[]} />);
    await userEvent.type(screen.getByRole("textbox"), "Hello");
    expect(onChange).toHaveBeenCalled();
  });

  it("number", async () => {
    const onChange = vi.fn();
    wrap(
      <NumberInput
        field={field({ name: "age", widget: "number", type: "integer" })}
        value=""
        onChange={onChange}
        entities={[]}
      />,
    );
    const input = screen.getByRole("spinbutton");
    await userEvent.clear(input);
    fireEvent.change(input, { target: { value: "12" } });
    expect(onChange).toHaveBeenLastCalledWith(12);
  });

  it("currency uses tenant currency", () => {
    wrap(
      <CurrencyInput
        field={field({ name: "price", widget: "currency", type: "decimal" })}
        value={15.5}
        onChange={() => undefined}
        entities={[]}
      />,
    );
    expect(screen.getByText("INR")).toBeInTheDocument();
  });

  it("percentage", async () => {
    const onChange = vi.fn();
    wrap(
      <PercentageInput
        field={field({ name: "discount", widget: "percentage", type: "decimal" })}
        value={15.5}
        onChange={onChange}
        entities={[]}
      />,
    );
    expect(screen.getByDisplayValue("15.5")).toBeInTheDocument();
  });

  it("date", async () => {
    const onChange = vi.fn();
    wrap(
      <DatePicker
        field={field({ name: "birth_date", widget: "date", type: "date" })}
        value="2026-08-15"
        onChange={onChange}
        entities={[]}
      />,
    );
    const input = screen.getByDisplayValue("2026-08-15");
    await userEvent.clear(input);
    await userEvent.type(input, "2026-08-16");
    expect(onChange).toHaveBeenCalled();
  });

  it("time", () => {
    wrap(
      <TimePicker
        field={field({ name: "appointment_time", widget: "time", type: "time" })}
        value="20:00"
        onChange={() => undefined}
        entities={[]}
      />,
    );
    expect(screen.getByDisplayValue("20:00")).toBeInTheDocument();
  });

  it("datetime converts tenant local", () => {
    wrap(
      <DateTimePicker
        field={field({
          name: "appointment_at",
          widget: "datetime",
          type: "datetime",
          widget_options: { timezone: "tenant" },
        })}
        value="2026-08-15T14:30:00Z"
        onChange={() => undefined}
        entities={[]}
      />,
    );
    expect(screen.getByDisplayValue("2026-08-15")).toBeInTheDocument();
    expect(screen.getByDisplayValue("20:00")).toBeInTheDocument();
  });

  it("color", async () => {
    const onChange = vi.fn();
    wrap(
      <ColorPicker
        field={field({ name: "brand_color", widget: "color" })}
        value="#112233"
        onChange={onChange}
        entities={[]}
      />,
    );
    await userEvent.clear(screen.getByPlaceholderText("#2563eb"));
    await userEvent.type(screen.getByPlaceholderText("#2563eb"), "#ff0000");
    expect(onChange).toHaveBeenCalled();
  });

  it("select", async () => {
    const onChange = vi.fn();
    wrap(
      <Select
        field={field({ name: "status", widget: "select", enum_values: ["Pending", "Confirmed"] })}
        value="Pending"
        onChange={onChange}
        entities={[]}
      />,
    );
    await userEvent.selectOptions(screen.getByRole("combobox"), "Confirmed");
    expect(onChange).toHaveBeenCalledWith("Confirmed");
  });

  it("multiselect", async () => {
    const onChange = vi.fn();
    wrap(
      <MultiSelect
        field={field({ name: "categories", widget: "multiselect", enum_values: ["A", "B"] })}
        value={[]}
        onChange={onChange}
        entities={[]}
      />,
    );
    await userEvent.click(screen.getByRole("checkbox", { name: "A" }));
    expect(onChange).toHaveBeenCalledWith(["A"]);
  });

  it("checkbox switch radio", async () => {
    const onChange = vi.fn();
    wrap(<Checkbox field={field({ name: "active", widget: "checkbox" })} value={false} onChange={onChange} entities={[]} />);
    await userEvent.click(screen.getByRole("checkbox"));
    expect(onChange).toHaveBeenCalledWith(true);
    wrap(<Switch field={field({ name: "enabled", widget: "switch" })} value={false} onChange={onChange} entities={[]} />);
    await userEvent.click(screen.getByRole("switch"));
    wrap(
      <Radio
        field={field({ name: "kind", widget: "radio", enum_values: ["a", "b"] })}
        value="a"
        onChange={onChange}
        entities={[]}
      />,
    );
    await userEvent.click(screen.getByRole("radio", { name: "b" }));
    expect(onChange).toHaveBeenCalledWith("b");
  });

  it("tags prevent duplicates", async () => {
    const onChange = vi.fn();
    wrap(<TagsInput field={field({ name: "tags", widget: "tags" })} value={["red"]} onChange={onChange} entities={[]} />);
    const input = screen.getByRole("textbox");
    await userEvent.type(input, "red{enter}");
    expect(onChange).not.toHaveBeenCalled();
    await userEvent.type(input, "blue{enter}");
    expect(onChange).toHaveBeenCalledWith(["red", "blue"]);
  });

  it("email phone url", () => {
    wrap(<EmailInput field={field({ name: "email", widget: "email" })} value="a@b.com" onChange={() => undefined} entities={[]} />);
    wrap(<PhoneInput field={field({ name: "phone", widget: "phone" })} value="+91 1" onChange={() => undefined} entities={[]} />);
    wrap(<UrlInput field={field({ name: "website", widget: "url" })} value="https://x" onChange={() => undefined} entities={[]} />);
    expect(screen.getByDisplayValue("a@b.com")).toBeInTheDocument();
  });

    it("json validation", async () => {
      const onChange = vi.fn();
      wrap(<JsonEditor field={field({ name: "metadata", widget: "json", type: "json" })} value={{}} onChange={onChange} entities={[]} />);
      const area = screen.getByRole("textbox");
      fireEvent.change(area, { target: { value: "{ invalid" } });
      expect(screen.getByRole("alert")).toBeInTheDocument();
    });

    it("renders relation file image and rich text", () => {
      wrap(
        <RelationPicker
          field={field({ name: "customer", widget: "relation", relation: "Customer" })}
          value=""
          onChange={() => undefined}
          entities={[]}
        />,
      );
      wrap(
        <FileUpload field={field({ name: "attachment", widget: "file" })} value="" onChange={() => undefined} entities={[]} />,
      );
      wrap(
        <ImageUpload field={field({ name: "image", widget: "image" })} value="" onChange={() => undefined} entities={[]} />,
      );
      wrap(
        <RichText field={field({ name: "body", widget: "rich_text" })} value="<p>Hi</p>" onChange={() => undefined} entities={[]} />,
      );
      expect(screen.getByLabelText("body")).toBeInTheDocument();
    });
});
