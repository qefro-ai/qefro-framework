import { MemoryRouter } from "react-router-dom";
import { FormLayout } from "../../components/forms/FormLayout";
import { formVisible, type UiEntity } from "../../api";
import { availableViews } from "../../metadata/views";
import { renderView } from "../../views/registry";
import "../../views";

function sampleRows(entity: UiEntity): Record<string, unknown>[] {
  const status = entity.fields.find((f) => f.name === "status");
  const values = status?.enum_values ?? ["Pending", "Confirmed"];
  return values.slice(0, 3).map((state, i) => ({
    id: `preview-${i}`,
    name: `${entity.label} ${i + 1}`,
    title: `${entity.label} ${i + 1}`,
    status: state,
    [status?.name || "status"]: state,
    _workflow: {
      current: state,
      transitions: values[i + 1] ? [{ name: "next", label: "Next", from: state, to: values[i + 1] }] : [],
    },
  }));
}

export default function ViewsPreview({ entity }: { entity: UiEntity }) {
  const views = availableViews(entity);
  const rows = sampleRows(entity);
  const fields = entity.fields.filter(formVisible).filter((f) => f.relation_kind !== "one_to_many");
  return (
    <MemoryRouter>
      <div className="card studio-preview">
        <h3>Views</h3>
        <p className="muted">
          Detected: {views.join(", ")}. Preview uses the production view registry.
        </p>
        {entity.views?.kanban || entity.views?.calendar || entity.views?.list || entity.views?.card ? (
          <pre className="muted">{JSON.stringify(entity.views, null, 2)}</pre>
        ) : null}
        {views.map((view) => (
          <section key={view} style={{ marginTop: "1rem" }}>
            <h4>{view}</h4>
            {view === "list" && fields.length ? (
              <FormLayout
                fields={fields.slice(0, 4)}
                values={rows[0] ?? {}}
                entities={[entity]}
                fieldErrors={{}}
                onChange={() => undefined}
              />
            ) : null}
            {renderView(view, {
              meta: entity,
              entities: [entity],
              slug: entity.slug,
              rows,
              total: rows.length,
              loading: false,
              onReload: () => undefined,
              onError: () => undefined,
            })}
          </section>
        ))}
      </div>
    </MemoryRouter>
  );
}
