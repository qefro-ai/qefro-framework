import { createElement, type ReactNode } from "react";
import { useParams } from "react-router-dom";
import type { UiEntity, UiField } from "../metadata/types";
import type { ViewKind } from "../metadata/types";
import { resolveEntity } from "../metadata/navigation";
import { defaultExtensions } from "../core/extensions";
import EntityList from "../pages/EntityList";
import EntityForm from "../pages/EntityForm";
import EntityDetail from "../pages/EntityDetail";
import { EntityCard, type CardChromeSpec } from "../components/views/EntityCard";
import { DashboardWidget, type DashboardWidgetCard } from "../pages/Dashboard";
import { PageHeader } from "../components/ui/PageHeader";
import { FieldValue } from "../components/fields/FieldValue";

function overridesFor(entity: UiEntity | string) {
  return defaultExtensions.entityOverrides(entity);
}

export function EntityListRenderer({
  entities,
  entity,
  view,
}: {
  entities: UiEntity[];
  entity?: string;
  view?: ViewKind;
}) {
  const { slug } = useParams();
  const meta = resolveEntity(entities, entity ?? slug);
  if (meta) {
    const Custom = overridesFor(meta).list;
    if (Custom) {
      return createElement(Custom, { entities, entity: meta.entity, meta, slug: meta.slug, view });
    }
  }
  return createElement(EntityList, { entities, entity, view });
}

export function EntityFormRenderer({ entities, entity }: { entities: UiEntity[]; entity?: string }) {
  const { slug } = useParams();
  const meta = resolveEntity(entities, entity ?? slug);
  if (meta) {
    const Custom = overridesFor(meta).form;
    if (Custom) return createElement(Custom, { entities, entity: meta.entity, meta, slug: meta.slug });
  }
  return createElement(EntityForm, { entities, entity });
}

export function EntityDetailRenderer({ entities, entity }: { entities: UiEntity[]; entity?: string }) {
  const { slug } = useParams();
  const meta = resolveEntity(entities, entity ?? slug);
  if (meta) {
    const Custom = overridesFor(meta).detail;
    if (Custom) return createElement(Custom, { entities, entity: meta.entity, meta, slug: meta.slug });
  }
  return createElement(EntityDetail, { entities, entity });
}

export function renderEntityCard(props: {
  meta: UiEntity;
  slug: string;
  row: Record<string, unknown>;
  spec?: CardChromeSpec;
  footer?: ReactNode;
  className?: string;
}) {
  const Custom = overridesFor(props.meta).card;
  if (Custom) return createElement(Custom, props);
  return createElement(EntityCard, props);
}

export function renderEntityHeader(props: Record<string, unknown> & { meta: UiEntity }) {
  const Custom = overridesFor(props.meta).header;
  if (Custom) return createElement(Custom, props);
  return createElement(PageHeader, props);
}

export function renderFieldValue(props: {
  meta: UiEntity;
  field?: UiField;
  fieldName?: string;
  row: Record<string, unknown>;
}) {
  const Custom = overridesFor(props.meta).field;
  if (Custom) return createElement(Custom, props);
  return createElement(FieldValue, props);
}

export function renderDashboardWidget(props: {
  card: DashboardWidgetCard;
  slug?: string;
  currency: string;
  locale: string;
  onSegment?: (card: DashboardWidgetCard, slug: string | undefined, label: string) => void;
}) {
  const kind = String(props.card.kind || "");
  const Custom = defaultExtensions.getDashboardWidget(kind);
  if (Custom) return createElement(Custom, props);
  return createElement(DashboardWidget, props);
}
