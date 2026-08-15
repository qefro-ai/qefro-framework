import { useState } from "react";
import { api } from "../../api";
import { publishAndReload } from "../StudioApp";

export default function FormulaEditor({
  entity,
  fields,
  functions,
  canPublish,
  onSaved,
}: {
  entity: string;
  fields: Array<Record<string, unknown>>;
  functions: string[];
  canPublish: boolean;
  onSaved: () => Promise<void>;
}) {
  const [name, setName] = useState(String(fields[0]?.name ?? ""));
  const field = fields.find((f) => f.name === name);
  const [formula, setFormula] = useState(String(field?.formula ?? ""));
  const [qty, setQty] = useState("2");
  const [rate, setRate] = useState("300");
  const [result, setResult] = useState<number | null>(null);
  const [error, setError] = useState("");

  async function preview() {
    const record: Record<string, unknown> = {
      quantity: Number(qty),
      rate: Number(rate),
    };
    const data = await api.studioFormulaPreview(formula, record);
    setResult(data.result);
  }

  return (
    <div className="form">
      <p className="muted">Allowed: {functions.join(" ")}</p>
      <label>
        Field
        <select
          value={name}
          onChange={(e) => {
            setName(e.target.value);
            const next = fields.find((f) => f.name === e.target.value);
            setFormula(String(next?.formula ?? ""));
          }}
        >
          {fields.map((f) => (
            <option key={String(f.name)}>{String(f.name)}</option>
          ))}
        </select>
      </label>
      <label>
        Formula
        <textarea value={formula} onChange={(e) => setFormula(e.target.value)} />
      </label>
      <div className="form-grid">
        <label>
          quantity
          <input value={qty} onChange={(e) => setQty(e.target.value)} />
        </label>
        <label>
          rate
          <input value={rate} onChange={(e) => setRate(e.target.value)} />
        </label>
      </div>
      {result != null ? <p>Preview result: {result} (UI only; server remains authoritative)</p> : null}
      {error ? <p className="error">{error}</p> : null}
      <div className="actions">
        <button type="button" className="ghost" onClick={() => preview().catch((e) => setError(e.message))}>
          Preview
        </button>
        <button
          type="button"
          disabled={!canPublish}
          onClick={() =>
            publishAndReload({
              kind: "entity.field.ui",
              target: entity,
              payload: { name, formula },
            })
              .then(onSaved)
              .catch((e) => setError(e.message))
          }
        >
          Publish
        </button>
      </div>
    </div>
  );
}
