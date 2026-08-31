import { useEffect, useMemo, useState } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";
import { api, type UiEntity } from "../../api";
import { can, groupedEntities } from "../StudioApp";
import SourceView from "../components/SourceView";
import CustomFieldsEditor from "../editors/CustomFieldsEditor";
import FormulaEditor from "../editors/FormulaEditor";
import FormPreview from "../preview/FormPreview";
import ViewsPreview from "../preview/ViewsPreview";
import ViewsEditor from "../editors/ViewsEditor";
import LayoutEditor from "../editors/LayoutEditor";
import SchedulingEditor from "../editors/SchedulingEditor";

type EntityPayload = {
  entity?: Record<string, unknown>;
  json?: string;
  yaml?: string;
  source?: string;
  source_managed?: boolean;
  referrers?: Array<{ entity: string; field: string }>;
  formula_functions?: string[];
  ui?: UiEntity;
};

export default function Entities({ caps }: { caps: string[] }) {
  const { entity } = useParams();
  const navigate = useNavigate();
  const [list, setList] = useState<Array<Record<string, unknown>>>([]);
  const [detail, setDetail] = useState<EntityPayload | null>(null);
  const [tab, setTab] = useState("fields");
  const [message, setMessage] = useState("");

  useEffect(() => {
    api.studioEntities().then((d) => setList(d.entities));
  }, []);

  useEffect(() => {
    if (!entity) {
      setDetail(null);
      return;
    }
    api.studioEntity(entity).then(setDetail);
  }, [entity]);

  const groups = useMemo(() => groupedEntities(list), [list]);

  if (!entity) {
    return (
      <div className="page">
        <h2>Entities</h2>
        {groups.map(([app, items]) => (
          <section key={app} className="card">
            <h3>
              <Link to={`/studio/apps/${app}`}>{app}</Link>
            </h3>
            <ul>
              {items.map((item) => (
                <li key={String(item.name)}>
                  <Link to={`/studio/entities/${item.name}`}>{String(item.label || item.name)}</Link>
                  {item.singleton ? <span className="muted"> · Singleton</span> : null}
                </li>
              ))}
            </ul>
          </section>
        ))}
      </div>
    );
  }

  const def = detail?.entity;
  const fields = ((def?.fields as Array<Record<string, unknown>>) ?? []).filter((f) => !f.system);
  const children = fields.filter((f) => f.type === "child_table");
  const relations = fields.filter((f) => f.type === "relation");
  const computed = fields.filter((f) => f.computed);
  const ui = detail?.ui;

  return (
    <div className="page">
      <p>
        <Link to="/studio/entities">Entities</Link>
        {def?.module ? (
          <>
            {" / "}
            <Link to={`/studio/apps/${def.module}`}>{String(def.module)}</Link>
          </>
        ) : null}
      </p>
      <h2>
        {String(def?.label || entity)}
        {def?.singleton ? <span className="badge">Singleton</span> : null}
        {def?.attachments ? <span className="badge">Attachments</span> : null}
      </h2>
      {def ? (
        <>
          <ul className="capability-list" aria-label="Capabilities">
            <li><label><input type="checkbox" checked={Boolean(def.attachments)} readOnly /> Attachments</label></li>
            <li><label><input type="checkbox" checked={Boolean(def.activity)} readOnly /> Activity</label></li>
            <li><label><input type="checkbox" checked={Boolean(def.audit)} readOnly /> Audit</label></li>
            <li><label><input type="checkbox" checked={Boolean(def.workflow)} readOnly /> Workflow</label></li>
            <li><label><input type="checkbox" checked={Boolean(def.scheduling)} readOnly /> Scheduling</label></li>
            <li><label><input type="checkbox" checked={Boolean(ui?.capabilities?.import)} readOnly /> Import</label></li>
          </ul>
          {ui?.capabilities?.import ? (
            <p className="muted">
              Matching:{" "}
              {fields.filter((f) => f.unique).map((f) => String(f.name)).join(", ") || "unique fields on EntityDef"}
            </p>
          ) : null}
        </>
      ) : null}
      {detail?.source_managed ? <p className="muted">Custom Rust operations remain source-managed.</p> : null}
      {detail?.referrers && detail.referrers.length > 0 ? (
        <p className="error">
          This entity is referenced by {detail.referrers.length} relationship
          {detail.referrers.length === 1 ? "" : "s"}.
        </p>
      ) : null}
      <div className="studio-tabs">
        {["fields", "custom fields", "relations", "child tables", "computed", "actions", "links", "public form", "layout", "views", "scheduling", "preview", "source"].map((name) => (
          <button
            key={name}
            className={tab === name ? "" : "ghost"}
            onClick={() => setTab(name)}
            type="button"
          >
            {name}
          </button>
        ))}
        <button className="ghost" type="button" onClick={() => navigate(`/studio/workflows/${entity}`)}>
          Workflow
        </button>
        <button className="ghost" type="button" onClick={() => navigate(`/studio/permissions/${entity}`)}>
          Permissions
        </button>
      </div>
      {message ? <p role="status">{message}</p> : null}
      {tab === "fields" && (
        <FieldEditor
          entity={entity}
          fields={fields}
          canEdit={can(caps, "studio.edit")}
          canPublish={can(caps, "studio.publish")}
          onSaved={async () => {
            setDetail(await api.studioEntity(entity));
            setMessage("Published.");
          }}
        />
      )}
      {tab === "custom fields" && (
        <CustomFieldsEditor
          entity={entity}
          fields={fields}
          ui={ui}
          canEdit={can(caps, "studio.edit")}
          canPublish={can(caps, "studio.publish")}
          onSaved={async () => {
            setDetail(await api.studioEntity(entity));
            setMessage("Published.");
          }}
        />
      )}
      {tab === "relations" && (
        <table>
          <thead>
            <tr>
              <th>Field</th>
              <th>Target</th>
            </tr>
          </thead>
          <tbody>
            {relations.map((field) => {
              const rel = field.relation as { target_entity?: string } | undefined;
              return (
                <tr key={String(field.name)}>
                  <td>{String(field.name)}</td>
                  <td>
                    {rel?.target_entity ? (
                      <Link to={`/studio/entities/${rel.target_entity}`}>{rel.target_entity}</Link>
                    ) : (
                      "—"
                    )}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      )}
      {tab === "child tables" && (
        <ul>
          {children.map((field) => {
            const rel = field.relation as { target_entity?: string } | undefined;
            const opts = (field.ui as { widget_options?: Record<string, unknown> } | undefined)
              ?.widget_options;
            return (
              <li key={String(field.name)}>
                <strong>{String(field.name)}</strong> →{" "}
                {rel?.target_entity ? (
                  <Link to={`/studio/entities/${rel.target_entity}`}>{rel.target_entity}</Link>
                ) : null}
                <div className="muted">
                  Editable: {opts?.editable === false ? "No" : "Yes"} · Add rows:{" "}
                  {opts?.addable === false ? "No" : "Yes"} · Delete rows:{" "}
                  {opts?.deletable === false ? "No" : "Yes"}
                </div>
              </li>
            );
          })}
          {children.length === 0 && <li className="muted">No child tables.</li>}
        </ul>
      )}
      {tab === "actions" && (
        <ul>
          {(((def?.actions as Array<Record<string, unknown>>) ?? []).length === 0) && (
            <li className="muted">No explicit actions. Workflow transitions still appear on the detail page.</li>
          )}
          {((def?.actions as Array<Record<string, unknown>>) ?? []).map((action) => (
            <li key={String(action.name)}>
              {String(action.label || action.name)} · operation {String(action.operation || action.name)}
              {action.confirmation ? " · confirmation required" : ""}
            </li>
          ))}
        </ul>
      )}
      {tab === "links" && (
        <ul>
          {(((def?.links as Array<Record<string, unknown>>) ?? []).length === 0) && (
            <li className="muted">Links are also derived from one-to-many relationships.</li>
          )}
          {((def?.links as Array<Record<string, unknown>>) ?? []).map((link) => (
            <li key={String(link.label)}>
              {String(link.label)} → <Link to={`/studio/entities/${link.entity}`}>{String(link.entity)}</Link> via{" "}
              {String(link.relation)}
            </li>
          ))}
        </ul>
      )}
      {tab === "public form" && (
        <div className="card">
          {def?.public_form ? (
            <>
              <p>Enabled: {(def.public_form as { enabled?: boolean }).enabled === false ? "No" : "Yes"}</p>
              <p>Slug: {String((def.public_form as { slug?: string }).slug)}</p>
              <p>Fields: {((def.public_form as { fields?: string[] }).fields ?? []).join(", ")}</p>
              {ui ? <FormPreview entity={{ ...ui, fields: ui.fields.filter((f) => ((def.public_form as { fields?: string[] }).fields ?? []).includes(f.name)) }} /> : null}
            </>
          ) : (
            <p className="muted">No public form on this entity.</p>
          )}
        </div>
      )}
      {tab === "computed" && (
        <FormulaEditor
          entity={entity}
          fields={computed}
          functions={detail?.formula_functions ?? []}
          canPublish={can(caps, "studio.publish")}
          onSaved={async () => setDetail(await api.studioEntity(entity))}
        />
      )}
      {tab === "layout" && ui && (
        <LayoutEditor
          entity={entity}
          ui={ui}
          canPublish={can(caps, "studio.publish")}
          onSaved={async () => {
            setDetail(await api.studioEntity(entity));
            setMessage("Published layout.");
          }}
        />
      )}
      {tab === "views" && ui && (
        <>
          <ViewsEditor
            entity={entity}
            ui={ui}
            canPublish={can(caps, "studio.publish")}
            onSaved={async () => {
              setDetail(await api.studioEntity(entity));
              setMessage("Published views.");
            }}
          />
          <ViewsPreview entity={ui} />
        </>
      )}
      {tab === "scheduling" && (
        <SchedulingEditor
          entity={entity}
          def={def}
          ui={ui}
          canPublish={can(caps, "studio.publish")}
          onSaved={async () => {
            setDetail(await api.studioEntity(entity));
            setMessage("Published scheduling.");
          }}
        />
      )}
      {tab === "preview" && ui && <FormPreview entity={ui} />}
      {tab === "source" && (
        <SourceView jsonText={detail?.json ?? ""} yamlText={detail?.yaml ?? ""} />
      )}
    </div>
  );
}
