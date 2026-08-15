import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { api } from "../api";

export default function GlobalSearch() {
  const [open, setOpen] = useState(false);
  const [q, setQ] = useState("");
  const [results, setResults] = useState<
    Array<{ entity: string; slug: string; id: string; label: string; snippet: string }>
  >([]);
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
    if (!open || !q.trim()) {
      setResults([]);
      return;
    }
    const handle = window.setTimeout(() => {
      api.search(q).then((d) => setResults(d.results)).catch(() => setResults([]));
    }, 150);
    return () => window.clearTimeout(handle);
  }, [open, q]);

  if (!open) return null;

  return (
    <div className="palette-backdrop" onClick={() => setOpen(false)}>
      <div className="palette" role="dialog" aria-label="Global search" onClick={(e) => e.stopPropagation()}>
        <input
          autoFocus
          placeholder="Search records…"
          value={q}
          onChange={(e) => setQ(e.target.value)}
        />
        <ul>
          {results.map((r) => (
            <li key={`${r.slug}-${r.id}`}>
              <button
                type="button"
                className="ghost"
                onClick={() => {
                  navigate(`/${r.slug}/${r.id}`);
                  setOpen(false);
                }}
              >
                <strong>{r.entity}</strong> {r.label}
                {r.snippet ? <span className="muted"> · {r.snippet}</span> : null}
              </button>
            </li>
          ))}
          {q && results.length === 0 ? <li className="muted">No matches.</li> : null}
        </ul>
      </div>
    </div>
  );
}
