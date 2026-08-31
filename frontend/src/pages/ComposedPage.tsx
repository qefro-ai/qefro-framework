import { useEffect, useMemo, useState } from "react";
import { useNavigate, useParams, useSearchParams } from "react-router-dom";
import { api, type UiEntity } from "../sdk/client";
import { Chart } from "../components/dashboards/Chart";
import { EmbeddedDetail } from "../components/pages/EmbeddedDetail";
import { EmbeddedEntityView } from "../components/pages/EmbeddedEntityView";
import { FilterBar } from "../components/filters/FilterBar";
import { Button } from "../components/ui/Button";
import { EmptyState, ErrorState, Skeleton } from "../components/ui/EmptyState";
import { PageHeader } from "../components/ui/PageHeader";
import { DashboardWidget, type DashboardWidgetCard } from "./Dashboard";
import { friendlyError } from "../friendlyError";
import { canCreate, canExport } from "../metadata/views";
import { useTenantTheme } from "../metadata/context";
import type { PageActionRef, PageDef, PageSection } from "../metadata/types";

function sectionKind(section: PageSection) {
  if (section.kind) return section.kind;
  if (section.dashboard || section.widget || section.card) return "widget";
  if (section.report) return "report";
  if (section.relation) return "related";
  if (section.action) return "action";
  return "entity_view";
}

function sizeClass(size?: string | null) {
  if (size === "xl") return "section-xl";
  if (size === "lg") return "section-lg";
  if (size === "sm") return "section-sm";
  return "section-md";
}

export default function ComposedPage({ entities }: { entities: UiEntity[] }) {
  const { name } = useParams();
  const [params, setParams] = useSearchParams();
  const navigate = useNavigate();
  const theme = useTenantTheme();
  const [page, setPage] = useState<PageDef | null>(null);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(true);
  const [tick, setTick] = useState(0);
  const [dashboards, setDashboards] = useState<Record<string, DashboardWidgetCard[]>>({});

  useEffect(() => {
    if (!name) return;
    setLoading(true);
    api
      .page(name)
      .then((def) => {
        setPage(def);
        setError("");
        const names = [
          ...new Set(
            (def.sections ?? [])
              .map((s) => s.dashboard)
              .filter((d): d is string => Boolean(d)),
          ),
        ];
        return Promise.all(
          names.map((dash) =>
            api.dashboard(dash).then((d) => [dash, d.cards as DashboardWidgetCard[]] as const),
          ),
        );
      })
      .then((pairs) => {
        if (!pairs) return;
        const next: Record<string, DashboardWidgetCard[]> = {};
        for (const [dash, cards] of pairs) next[dash] = cards;
        setDashboards(next);
      })
      .catch((err) => setError(friendlyError(err)))
      .finally(() => setLoading(false));
  }, [name, tick]);

  const tab = params.get("tab") || page?.tabs?.[0]?.name || "";
  const contextKey = page?.context_param || "id";
  const selectedId = params.get(contextKey) || params.get("id") || "";

  const visibleSections = useMemo(() => {
    const sections = page?.sections ?? [];
    if (!page?.tabs?.length) return sections;
    return sections.filter((s) => !s.tab || s.tab === tab);
  }, [page, tab]);

  const sharedFields = useMemo(() => {
    const names = new Set(page?.filters ?? []);
    if (names.size === 0) return [];
    const fields = [];
    const seen = new Set<string>();
    for (const entity of entities) {
      for (const field of entity.fields) {
        if (names.has(field.name) && (field.filter || field.filterable) && !seen.has(field.name)) {
          seen.add(field.name);
          fields.push(field);
        }
      }
    }
    return fields;
  }, [page, entities]);

  const extra = useMemo(() => {
    const q = new URLSearchParams();
    for (const [key, value] of params.entries()) {
      if (["tab", "id", contextKey, "view"].includes(key)) continue;
      if (value) q.set(key, value);
    }
    return q;
  }, [params, contextKey]);

  if (loading && !page) return <Skeleton variant="dashboard" />;
  if (error && !page) return <ErrorState message={error} onRetry={() => setTick((n) => n + 1)} />;
  if (!page) return <ErrorState message="Unknown page." />;

  const layout = page.layout || "stack";
  const isSplit = layout === "split";
  const master = visibleSections.filter((s) => s.pane === "master");
  const detail = visibleSections.filter((s) => s.pane === "detail");
  const main = visibleSections.filter((s) => !s.pane || s.pane === "main");

  function setTab(next: string) {
    const copy = new URLSearchParams(params);
    copy.set("tab", next);
    setParams(copy);
  }

  function setSelected(id: string) {
    const copy = new URLSearchParams(params);
    copy.set(contextKey, id);
    setParams(copy);
  }

  async function runPageAction(action: PageActionRef) {
    const meta = entities.find((e) => e.entity === action.entity);
    if (!meta) return;
    if (action.action === "create" && canCreate(meta)) {
      navigate(`/${meta.slug}/new`);
      return;
    }
    if (action.action === "export" && canExport(meta)) {
      await api.exportCsv(meta.slug, extra);
      return;
    }
    if (action.action === "refresh") {
      setTick((n) => n + 1);
    }
  }

  const filterEntity = entities.find((e) => e.fields.some((f) => sharedFields.some((sf) => sf.name === f.name)));

  return (
    <div className={`page composed-page layout-${layout}`}>
      <PageHeader
        kicker="Workspace"
        title={page.label}
        description={page.description || undefined}
        actions={
          <div className="actions">
            {(page.actions ?? [])
              .filter((action) => {
                const meta = entities.find((e) => e.entity === action.entity);
                if (!meta) return false;
                if (action.action === "create") return canCreate(meta);
                if (action.action === "export") return canExport(meta);
                return true;
              })
              .map((action) => (
              <Button
                key={`${action.entity}-${action.action}`}
                variant={action.action === "create" ? "filled" : "outlined"}
                onClick={() => void runPageAction(action)}
              >
                {action.label || action.action}
              </Button>
            ))}
          </div>
        }
      />
      {sharedFields.length > 0 && filterEntity ? (
        <FilterBar
          entity={filterEntity.entity}
          fields={sharedFields}
          entities={entities}
          params={params}
          onChange={(key, value) => {
            const copy = new URLSearchParams(params);
            if (value) copy.set(key, value);
            else copy.delete(key);
            setParams(copy);
          }}
          onReplace={setParams}
        />
      ) : null}
      {page.tabs && page.tabs.length > 0 ? (
        <div className="tabs" role="tablist" aria-label={`${page.label} tabs`}>
          {page.tabs.map((item) => (
            <button
              key={item.name}
              type="button"
              role="tab"
              aria-selected={tab === item.name}
              className={tab === item.name ? "active" : undefined}
              onClick={() => setTab(item.name)}
            >
              {item.label}
            </button>
          ))}
        </div>
      ) : null}
      {isSplit ? (
        <div className="page-split">
          <div className="page-split-master">
            {master.map((section) => (
              <PageSectionFrame
                key={section.name || section.title}
                section={section}
                entities={entities}
                extra={extra}
                dashboards={dashboards}
                selectedId={selectedId}
                onSelect={setSelected}
                contextId={selectedId}
                currency={theme.currency}
                locale={theme.locale}
              />
            ))}
          </div>
          <div className="page-split-detail">
            {detail.length === 0 && master[0] ? (
              <EmbeddedDetail
                entities={entities}
                slug={entities.find((e) => e.entity === master[0].entity)?.slug || ""}
                id={selectedId}
              />
            ) : (
              detail.map((section) => (
                <PageSectionFrame
                  key={section.name || section.title}
                  section={section}
                  entities={entities}
                  extra={extra}
                  dashboards={dashboards}
                  selectedId={selectedId}
                  onSelect={setSelected}
                  contextId={selectedId}
                  currency={theme.currency}
                  locale={theme.locale}
                />
              ))
            )}
          </div>
        </div>
      ) : (
        <div className={`page-layout page-layout-${layout}`}>
          {main.map((section) => (
            <PageSectionFrame
              key={section.name || section.title}
              section={section}
              entities={entities}
              extra={extra}
              dashboards={dashboards}
              selectedId={selectedId}
              onSelect={setSelected}
              contextId={selectedId}
              currency={theme.currency}
              locale={theme.locale}
            />
          ))}
        </div>
      )}
    </div>
  );
}

function PageSectionFrame({
  section,
  entities,
  extra,
  dashboards,
  selectedId,
  onSelect,
  contextId,
  currency,
  locale,
}: {
  section: PageSection;
  entities: UiEntity[];
  extra: URLSearchParams;
  dashboards: Record<string, DashboardWidgetCard[]>;
  selectedId?: string;
  onSelect?: (id: string) => void;
  contextId?: string;
  currency: string;
  locale: string;
}) {
  return (
    <section className={`page-section panel ${sizeClass(section.size)}`} aria-label={section.title}>
      <h3 className="section-title">{section.title}</h3>
      <SectionBody
        section={section}
        entities={entities}
        extra={extra}
        dashboards={dashboards}
        selectedId={selectedId}
        onSelect={onSelect}
        contextId={contextId}
        currency={currency}
        locale={locale}
      />
    </section>
  );
}

function SectionBody({
  section,
  entities,
  extra,
  dashboards,
  selectedId,
  onSelect,
  contextId,
  currency,
  locale,
}: {
  section: PageSection;
  entities: UiEntity[];
  extra: URLSearchParams;
  dashboards: Record<string, DashboardWidgetCard[]>;
  selectedId?: string;
  onSelect?: (id: string) => void;
  contextId?: string;
  currency: string;
  locale: string;
}) {
  const kind = sectionKind(section);
  if (kind === "widget") {
    return (
      <WidgetSection
        section={section}
        entities={entities}
        dashboards={dashboards}
        extra={extra}
        currency={currency}
        locale={locale}
      />
    );
  }
  if (kind === "report" && section.report) {
    return <ReportSection name={section.report} extra={extra} />;
  }
  if (kind === "activity") {
    const meta = entities.find((e) => e.entity === section.entity);
    if (!meta || !contextId) {
      return <EmptyState title="Select a record to see activity" />;
    }
    return <EmbeddedDetail entities={entities} slug={meta.slug} id={contextId} showActivity />;
  }
  if (kind === "attachments") {
    const meta = entities.find((e) => e.entity === section.entity);
    if (!meta || !contextId) return <EmptyState title="Select a record to see files" />;
    return <EmbeddedDetail entities={entities} slug={meta.slug} id={contextId} showAttachments />;
  }
  if (kind === "related") {
    return (
      <RelatedSection
        section={section}
        entities={entities}
        extra={extra}
        contextId={contextId}
        selectedId={selectedId}
        onSelect={onSelect}
      />
    );
  }
  const meta = entities.find((e) => e.entity === section.entity);
  if (!meta) return <ErrorState message={`Unknown entity ${section.entity || ""}`} />;
  return (
    <EmbeddedEntityView
      entities={entities}
      slug={meta.slug}
      view={section.view}
      query={section.query}
      extra={extra}
      compact
      selectedId={selectedId}
      onSelect={onSelect}
      emptyAction={
        canCreate(meta)
          ? { label: `Create ${meta.label}`, to: `/${meta.slug}/new` }
          : undefined
      }
    />
  );
}

function RelatedSection({
  section,
  entities,
  extra,
  contextId,
  selectedId,
  onSelect,
}: {
  section: PageSection;
  entities: UiEntity[];
  extra: URLSearchParams;
  contextId?: string;
  selectedId?: string;
  onSelect?: (id: string) => void;
}) {
  const parent = entities.find((e) => e.entity === section.entity);
  const field = parent?.fields.find((f) => f.name === section.relation);
  const targetName = field?.relation || field?.child_entity;
  const target = entities.find((e) => e.entity === targetName);
  if (!parent || !target) {
    return <ErrorState message="Unknown related entity." />;
  }
  if (!contextId) {
    return <EmptyState title={`Select a ${parent.label.toLowerCase()}`} />;
  }
  const fk = field?.inverse_field || section.relation || "";
  const q = new URLSearchParams(extra);
  if (fk) q.set(fk, contextId);
  return (
    <EmbeddedEntityView
      entities={entities}
      slug={target.slug}
      view={section.view || "list"}
      extra={q}
      compact
      selectedId={selectedId}
      onSelect={onSelect}
      emptyAction={
        canCreate(target)
          ? { label: `Create ${target.label}`, to: `/${target.slug}/new?${fk}=${contextId}` }
          : undefined
      }
    />
  );
}

function WidgetSection({
  section,
  entities,
  dashboards,
  extra,
  currency,
  locale,
}: {
  section: PageSection;
  entities: UiEntity[];
  dashboards: Record<string, DashboardWidgetCard[]>;
  extra: URLSearchParams;
  currency: string;
  locale: string;
}) {
  const [card, setCard] = useState<DashboardWidgetCard | null>(null);
  const [error, setError] = useState("");
  const [tick, setTick] = useState(0);
  const dashName = section.dashboard || "";
  const want = section.widget || section.title;

  useEffect(() => {
    if (dashName && dashboards[dashName]) {
      const found = dashboards[dashName].find((c) => c.title === want) ?? null;
      setCard(found);
      setError(found ? "" : "Unable to load data");
      return;
    }
    if (!dashName) {
      setError("Unable to load data");
      return;
    }
    api
      .dashboard(dashName)
      .then((d) => {
        const found = (d.cards as DashboardWidgetCard[]).find((c) => c.title === want) ?? null;
        setCard(found);
        setError(found ? "" : "Unable to load data");
      })
      .catch((err) => setError(friendlyError(err)));
  }, [dashName, dashboards, want, extra.toString(), tick]);

  if (error && !card) {
    return <ErrorState message={error} onRetry={() => setTick((n) => n + 1)} />;
  }
  if (!card) return <Skeleton variant="dashboard" rows={1} />;
  const slug = entities.find((e) => e.entity === card.entity)?.slug;
  return (
    <DashboardWidget
      card={card}
      slug={slug}
      currency={currency}
      locale={locale}
    />
  );
}

function ReportSection({ name, extra }: { name: string; extra: URLSearchParams }) {
  const [result, setResult] = useState<{
    label?: string;
    chart?: string;
    rows: Array<Record<string, unknown>>;
    series?: Array<{ label: string; value: number }>;
  } | null>(null);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(true);
  const [tick, setTick] = useState(0);

  useEffect(() => {
    const filters = [];
    for (const [field, value] of extra.entries()) {
      if (value) filters.push({ field, value });
    }
    setLoading(true);
    api
      .runReport(name, { filters })
      .then((data) => {
        setResult(data);
        setError("");
      })
      .catch((err) => setError(friendlyError(err)))
      .finally(() => setLoading(false));
  }, [name, extra.toString(), tick]);

  if (loading) return <Skeleton />;
  if (error) return <ErrorState message={error} onRetry={() => setTick((n) => n + 1)} />;
  if (!result || (result.rows ?? []).length === 0) {
    return <EmptyState title="No report rows" />;
  }
  return (
    <div className="report-embed">
      {result.series && result.series.length > 0 ? (
        <Chart
          kind={result.chart || "bar"}
          series={result.series.map((s) => ({ label: String(s.label), value: Number(s.value) }))}
        />
      ) : null}
      <table className="data">
        <thead>
          <tr>
            {Object.keys(result.rows[0] ?? {}).map((k) => (
              <th key={k}>{k}</th>
            ))}
          </tr>
        </thead>
        <tbody>
          {result.rows.slice(0, 12).map((row, i) => (
            <tr key={i}>
              {Object.values(row).map((v, j) => (
                <td key={j}>{String(v ?? "")}</td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
