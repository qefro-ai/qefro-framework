import { useMemo, useState } from "react";
import { FormLayout } from "../../components/forms/FormLayout";
import { formVisible, type UiEntity } from "../../api";
import { previewFormula } from "../../metadata/formula";

export default function FormPreview({ entity }: { entity: UiEntity }) {
  const fields = useMemo(
    () => entity.fields.filter(formVisible).filter((f) => f.relation_kind !== "one_to_many"),
    [entity],
  );
  const [values, setValues] = useState<Record<string, unknown>>({});

  return (
    <div className="card">
      <h3>{entity.label} preview</h3>
      <p className="muted">Same generic form renderer as the application UI.</p>
      <FormLayout
        fields={fields}
        values={values}
        entities={[entity]}
        fieldErrors={{}}
        onChange={(name, value) =>
          setValues((prev) => {
            const next = { ...prev, [name]: value };
            for (const field of fields) {
              if (field.computed && field.formula) {
                const preview = previewFormula(field.formula, next);
                if (preview != null) next[field.name] = preview;
              }
            }
            return next;
          })
        }
      />
    </div>
  );
}
