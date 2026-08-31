import React, { useEffect, useState } from "react";
import type { UiField, ViewSection } from "../../metadata/types";
import { fieldReadonly, fieldRequired } from "../../metadata/conditions";
import { fieldSectionTitle, fieldTab, resolveLayout, tabHasError } from "../../metadata/layout";
import { renderWidget } from "../../metadata/registry";
import type { UiEntity } from "../../api";

export function FormLayout({
  fields,
  values,
  entities,
  fieldErrors,
  onChange,
  layout,
  focusField,
  focusSeq,
}: {
  fields: UiField[];
  values: Record<string, unknown>;
  entities: UiEntity[];
  fieldErrors: Record<string, string>;
  onChange: (name: string, value: unknown) => void;
  layout?: ViewSection[];
  focusField?: string | null;
  focusSeq?: number;
}) {
  const resolved = resolveLayout(fields, layout, values);
  const tabs = resolved.tabs.filter(Boolean);
  const [active, setActive] = useState(tabs[0] ?? "");
  const [forcedOpen, setForcedOpen] = useState<string | null>(null);

  useEffect(() => {
    if (!focusField) return;
    const tab = fieldTab(resolved, focusField);
    if (tab) setActive(tab);
    const section = fieldSectionTitle(resolved, focusField);
    if (section) setForcedOpen(section);
  }, [focusField, resolved]);

  useEffect(() => {
    if (!focusField) return;
    const el =
      document.getElementById(`field-${focusField}`) || document.querySelector(`[data-field="${focusField}"]`);
    if (!(el instanceof HTMLElement)) return;
    el.scrollIntoView?.({ behavior: "smooth", block: "center" });
    const input = el.matches("input, textarea, select, button")
      ? el
      : el.querySelector("input, textarea, select, button");
    if (input instanceof HTMLElement) input.focus();
  }, [focusField, resolved, forcedOpen, active, focusSeq]);

  const namedTabs = tabs.length ? tabs : [""];
  const showTabs = tabs.length > 1;

  return (
    <div>
      {showTabs ? (
        <div className="tabs" role="tablist">
          {namedTabs.map((tab) => {
            const invalid = tabHasError(resolved, tab, fieldErrors);
            return (
              <button
                key={tab}
                type="button"
                role="tab"
                aria-selected={active === tab}
                className={`${active === tab ? "is-active" : "ghost"}${invalid ? " has-error" : ""}`}
                onClick={() => setActive(tab)}
              >
                {tab}
                {invalid ? (
                  <span className="tab-error" aria-label="Contains errors">
                    •
                  </span>
                ) : null}
              </button>
            );
          })}
        </div>
      ) : null}
      {resolved.sections
        .filter((section) => (showTabs ? section.tab === active : true))
        .map((section) => (
            <Section
            key={`${section.tab}-${section.title || "default"}`}
            title={section.title}
            collapsedDefault={Boolean(section.collapsed)}
            forceOpen={forcedOpen === section.title}
          >
            {section.columns.length > 1 ? (
              <div className="form-columns">
                {section.columns.map((col, i) => (
                  <div key={i} className="form-column">
                    <div className="form-grid">
                      {col.fields.map((field) => (
                        <FieldCell
                          key={field.name}
                          field={field}
                          values={values}
                          entities={entities}
                          fieldErrors={fieldErrors}
                          onChange={onChange}
                        />
                      ))}
                    </div>
                  </div>
                ))}
              </div>
            ) : (
              <div className="form-grid">
                {section.columns[0]?.fields.map((field) => (
                  <FieldCell
                    key={field.name}
                    field={field}
                    values={values}
                    entities={entities}
                    fieldErrors={fieldErrors}
                    onChange={onChange}
                  />
                ))}
              </div>
            )}
          </Section>
        ))}
      <BalanceHint values={values} />
    </div>
  );
}

function FieldCell({
  field,
  values,
  entities,
  fieldErrors,
  onChange,
}: {
  field: UiField;
  values: Record<string, unknown>;
  entities: UiEntity[];
  fieldErrors: Record<string, string>;
  onChange: (name: string, value: unknown) => void;
}) {
  const readonly = fieldReadonly(field, values) || Boolean(field.computed);
  const required = fieldRequired(field, values);
  const width = field.width || "full";
  const inputId = `field-${field.name}`;
  const help = field.help || field.help_text || field.description;
  const isBool = field.widget === "checkbox" || field.widget === "switch";
  const invalid = Boolean(fieldErrors[field.name]);
  const fieldForWidget = required === field.required ? field : { ...field, required };
  return (
    <div
      data-field={field.name}
      className={`field-cell width-${width}${invalid ? " is-invalid" : ""}${field.computed ? " is-computed" : ""}`}
    >
      {isBool ? null : (
        <label htmlFor={inputId}>
          {field.label}
          {required ? " *" : ""}
          {field.computed ? <span className="computed-hint"> calculated</span> : null}
        </label>
      )}
      {field.placeholder && !isBool ? <span className="sr-only">{field.placeholder}</span> : null}
      {renderWidget({
        field: fieldForWidget,
        value: values[field.name],
        entities,
        disabled: readonly,
        id: inputId,
        invalid,
        fieldErrors,
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
}

function Section({
  title,
  collapsedDefault,
  forceOpen,
  children,
}: {
  title: string;
  collapsedDefault: boolean;
  forceOpen: boolean;
  children: React.ReactNode;
}) {
  const [collapsed, setCollapsed] = useState(collapsedDefault);
  useEffect(() => {
    if (forceOpen) setCollapsed(false);
  }, [forceOpen]);
  const hidden = collapsed && !forceOpen;
  return (
    <fieldset className={hidden ? "is-collapsed" : ""}>
      {title ? (
        <legend>
          <button
            type="button"
            className="section-toggle"
            aria-expanded={!hidden}
            onClick={() => setCollapsed((v) => !v)}
          >
            {title}
          </button>
        </legend>
      ) : null}
      {hidden ? null : children}
    </fieldset>
  );
}

function moneyAmount(value: unknown): number | null {
  if (value == null || value === "") return null;
  const n = Number(value);
  return Number.isFinite(n) ? n : null;
}

function BalanceHint({ values }: { values: Record<string, unknown> }) {
  const debit = moneyAmount(values.total_debit);
  const credit = moneyAmount(values.total_credit);
  if (debit == null || credit == null) return null;
  const diff = Math.round((debit - credit) * 100) / 100;
  const balanced = diff === 0;
  return (
    <p className={`ledger-balance${balanced ? " is-balanced" : " is-unbalanced"}`} role="status">
      {balanced
        ? `Balanced ✓  Debit: ${debit.toFixed(2)}  Credit: ${credit.toFixed(2)}`
        : `Not balanced  Debit: ${debit.toFixed(2)}  Credit: ${credit.toFixed(2)}  Difference: ${Math.abs(diff).toFixed(2)}`}
    </p>
  );
}

function nestedErrors(fieldErrors: Record<string, string>, name: string) {
  const prefix = `${name}.`;
  return Object.entries(fieldErrors)
    .filter(([key]) => key.startsWith(prefix))
    .map(([key, message]) => `${key}: ${message}`);
}
