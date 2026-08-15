import { useState } from "react";

export default function SourceView({ jsonText, yamlText }: { jsonText: string; yamlText: string }) {
  const [tab, setTab] = useState<"json" | "yaml">("json");
  return (
    <div>
      <div className="studio-tabs" role="tablist">
        <button type="button" className={tab === "json" ? "" : "ghost"} onClick={() => setTab("json")}>
          JSON
        </button>
        <button type="button" className={tab === "yaml" ? "" : "ghost"} onClick={() => setTab("yaml")}>
          YAML
        </button>
      </div>
      <pre className="mono source-view">{tab === "json" ? jsonText : yamlText}</pre>
    </div>
  );
}
