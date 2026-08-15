import type { ReactNode } from "react";
import type { UiEntity, UiField } from "../metadata/types";

export type WidgetProps = {
  field: UiField;
  value: unknown;
  onChange: (value: unknown) => void;
  entities: UiEntity[];
  disabled?: boolean;
  id?: string;
};

export type Widget = (props: WidgetProps) => ReactNode;

const registry: Record<string, Widget> = {};

export function registerWidget(name: string, widget: Widget) {
  registry[name] = widget;
}

export function renderWidget(props: WidgetProps) {
  const key = String(props.field.widget || props.field.type || "text").toLowerCase();
  const Widget = registry[key] || registry.text;
  return Widget(props);
}

export function registeredWidgets() {
  return Object.keys(registry);
}
