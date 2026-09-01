import type { ReactNode } from "react";
import { Link } from "react-router-dom";
import { FieldValue } from "../fields/FieldValue";
import { displayValue } from "../../metadata/views";
import type { UiEntity } from "../../metadata/types";

export type CardChromeSpec = {
  title?: string;
  subtitle?: string;
  image?: string;
  fields?: string[];
};

export function EntityCard({
  meta,
  slug,
  row,
  spec,
  footer,
  className,
}: {
  meta: UiEntity;
  slug: string;
  row: Record<string, unknown>;
  spec?: CardChromeSpec;
  footer?: ReactNode;
  className?: string;
}) {
  const titleField = spec?.title || meta.display_field || "name";
  const subtitleField = spec?.subtitle;
  const extra = (spec?.fields ?? []).filter((name) => name !== titleField && name !== subtitleField);
  const imageField = spec?.image;
  const imageValue = imageField ? row[imageField] : undefined;
  const title = displayValue(row, titleField);

  return (
    <article className={className || "entity-card"}>
      {imageValue ? (
        <img
          src={`/api/v1/files/${encodeURIComponent(String(imageValue))}`}
          alt=""
          className="entity-card-image"
        />
      ) : null}
      <Link to={`/${slug}/${row.id}`} className="entity-card-title">
        <strong>{title}</strong>
      </Link>
      {subtitleField ? (
        <div className="entity-card-subtitle muted">
          <FieldValue row={row} field={meta.fields.find((f) => f.name === subtitleField)} fieldName={subtitleField} />
        </div>
      ) : null}
      {extra.map((name) => {
        const field = meta.fields.find((f) => f.name === name);
        return (
          <div key={name} className="entity-card-field">
            {field?.label ? <span className="muted">{field.label}</span> : null}
            <FieldValue row={row} field={field} fieldName={name} />
          </div>
        );
      })}
      {footer}
    </article>
  );
}
