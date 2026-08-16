import React, { useState } from "react";
import type { UiField } from "../../metadata/types";
import { fieldReadonly, fieldVisible, matchesWhen } from "../../metadata/conditions";
import type { UiWhen } from "../../metadata/types";
import { renderWidget } from "../../metadata/registry";
import type { UiEntity } from "../../api";

export function FormLayout({
  fields,
  values,
  entities,
  fieldErrors,
  onChange,
  sectionRules,
}: {
  fields: UiField[];
  values: Record<string, unknown>;
  entities: UiEntity[];
  fieldErrors: Record<string, string>;
  onChange: (name: string, value: unknown) => void;
  sectionRules?: Array<{ title: string; visible_when?: UiWhen }>;
}) {
  const visible = fields.filter((f) => fieldVisible(f, values) && f.relation_kind !== "one_to_many");
  const tabs = unique(visible.map((f) => f.tab).filter(Boolean) as string[]);
  const activeTabs = tabs.length ? tabs : [""];

  return (
    <Tabbed sections={activeTabs}>
      {(tab) => {
        const inTab = visible.filter((f) => (f.tab ?? "") === tab);
        const sections = groupBy(inTab, (f) => f.section ?? "").filter(([section]) => {
          const rule = sectionRules?.find((s) => s.title === section);
          if (!rule?.visible_when) return true;
          return matchesWhen(rule.visible_when, values);
        });
        return sections.map(([section, sectionFields]) => (
          <Section key={`${tab}-${section || "default"}`} title={section} fields={sectionFields}>
            <div className="form-grid">
              {sectionFields.map((field) => {
                const readonly = fieldReadonly(field, values);
                const width = field.width || "full";
                const inputId = `field-${field.name}`;
                const help = field.help || field.help_text || field.description;
                const isBool = field.widget === "checkbox" || field.widget === "switch";
                const invalid = Boolean(fieldErrors[field.name]);
                return (
                  <div key={field.name} className={`field-cell width-${width}`}>
                    {isBool ? null : (
                      <label htmlFor={inputId}>
                        {field.label}
                        {field.required ? " *" : ""}
                      </label>
                    )}
                    {field.placeholder && !isBool ? <span className="sr-only">{field.placeholder}</span> : null}
                    {renderWidget({
                      field,
                      value: values[field.name],
                      entities,
                      disabled: readonly,
                      id: inputId,
                      invalid,
                      onChange: (value) => onChange(field.name, value),
                    })}
                    {help && (
                      <span id={`${field.name}-help`} className="muted">
                        {help}
                      </span>
                    )}
                    {fieldErrors[field.name] && (
                      <span id={`${field.name}-error`} className="error" role="alert">
                        {fieldErrors[field.name]}
                      </span>
                    )}
                    {nestedErrors(fieldErrors, field.name).map((msg) => (
                      <span key={msg} className="error" role="alert">
                        {msg}
                      </span>
                    ))}
                  </div>
                );
              })}
            </div>
          </Section>
        ));
      }}
    </Tabbed>
  );
}

function Section({
  title,
  fields,
  children,
}: {
  title: string;
  fields: UiField[];
  children: React.ReactNode;
}) {
  const [collapsed, setCollapsed] = useState(() => fields.some((f) => f.widget_options?.collapsed));
  return (
    <fieldset className={collapsed ? "is-collapsed" : ""}>
      {title ? (
        <legend>
          <button
            type="button"
            className="section-toggle"
            aria-expanded={!collapsed}
            onClick={() => setCollapsed((v) => !v)}
          >
            {title}
          </button>
        </legend>
      ) : null}
      {collapsed ? null : children}
    </fieldset>
  );
}

function Tabbed({
  sections,
  children,
}: {
  sections: string[];
  children: (tab: string) => React.ReactNode;
}) {
  const named = sections.filter(Boolean);
  const [active, setActive] = useState(named[0] ?? "");
  if (named.length <= 1) return <>{children(named[0] ?? "")}</>;
  return (
    <div>
      <div className="tabs" role="tablist">
        {named.map((tab) => (
          <button
            key={tab}
            type="button"
            role="tab"
            aria-selected={active === tab}
            className={active === tab ? "" : "ghost"}
            onClick={() => setActive(tab)}
          >
            {tab}
          </button>
        ))}
      </div>
      <div role="tabpanel">{children(active)}</div>
    </div>
  );
}

function unique(items: string[]) {
  return [...new Set(items)];
}

function groupBy<T>(items: T[], key: (item: T) => string): Array<[string, T[]]> {
  const map = new Map<string, T[]>();
  for (const item of items) {
    const k = key(item);
    const list = map.get(k) ?? [];
    list.push(item);
    map.set(k, list);
  }
  return Array.from(map.entries());
}

function nestedErrors(fieldErrors: Record<string, string>, name: string) {
  const prefix = `${name}.`;
  return Object.entries(fieldErrors)
    .filter(([key]) => key.startsWith(prefix))
    .map(([key, message]) => `${key}: ${message}`);
}
