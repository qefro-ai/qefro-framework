import { useEffect, useMemo, useState } from "react";
import { useNavigate } from "react-router-dom";
import { api } from "../../api";

export default function CommandPalette({ caps }: { caps: string[] }) {
  const [open, setOpen] = useState(false);
  const [q, setQ] = useState("");
  const [results, setResults] = useState<Array<{ kind: string; name: string; label?: string; entity?: string }>>([]);
  const navigate = useNavigate();

  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setOpen((v) => !v);
      }
      if (e.key === "Escape") setOpen(false);
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  useEffect(() => {
    if (!open || !q) {
      setResults([]);
      return;
    }
    const handle = window.setTimeout(() => {
      api.studioSearch(q).then((d) => setResults(d.results)).catch(() => setResults([]));
    }, 150);
    return () => window.clearTimeout(handle);
  }, [open, q]);

  const commands = useMemo(
    () =>
      [
        { label: "Open Overview", path: "/studio" },
        { label: "Open Apps", path: "/studio/apps" },
        { label: "Open Entities", path: "/studio/entities" },
        { label: "Open Workflows", path: "/studio/workflows" },
        { label: "Open Permissions", path: "/studio/permissions" },
        { label: "Open Notifications", path: "/studio/notifications" },
        { label: "Open Webhooks", path: "/studio/webhooks" },
        { label: "Open Automations", path: "/studio/automations" },
        { label: "Open Public Forms", path: "/studio/public-forms" },
        { label: "Open Reports", path: "/studio/reports" },
        { label: "Open Dashboards", path: "/studio/dashboards" },
        { label: "Open Pages", path: "/studio/pages" },
        { label: "Open Tenant settings", path: "/studio/system" },
      ].filter((c) => c.label.toLowerCase().includes(q.toLowerCase()) || !q),
    [q],
  );

  if (!open || !caps.includes("studio.view")) return null;

  return (
    <div className="palette-backdrop" onClick={() => setOpen(false)}>
      <div className="palette" role="dialog" aria-label="Studio command palette" onClick={(e) => e.stopPropagation()}>
        <input
          autoFocus
          placeholder="Search metadata or run a command…"
          value={q}
          onChange={(e) => setQ(e.target.value)}
        />
        <ul>
          {commands.map((c) => (
            <li key={c.path}>
              <button
                type="button"
                className="ghost"
                onClick={() => {
                  navigate(c.path);
                  setOpen(false);
                }}
              >
                {c.label}
              </button>
            </li>
          ))}
          {results.map((r, i) => (
            <li key={`${r.kind}-${r.name}-${i}`}>
              <button
                type="button"
                className="ghost"
                onClick={() => {
                  navigate(pathFor(r));
                  setOpen(false);
                }}
              >
                {r.kind}: {r.label || r.name}
              </button>
            </li>
          ))}
        </ul>
      </div>
    </div>
  );
}

function pathFor(r: { kind: string; name: string; entity?: string }) {
  switch (r.kind) {
    case "app":
      return `/studio/apps/${r.name}`;
    case "entity":
    case "child_table":
      return `/studio/entities/${r.entity || r.name}`;
    case "workflow":
      return `/studio/workflows/${r.entity || r.name}`;
    case "permission":
      return `/studio/permissions/${r.entity || r.name}`;
    case "report":
      return `/studio/reports/${r.name}`;
    case "dashboard":
      return `/studio/dashboards/${r.name}`;
    case "page":
      return `/studio/pages/${r.name}`;
    case "print_format":
      return `/studio/print-formats/${r.name}`;
    default:
      return "/studio";
  }
}
