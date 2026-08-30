import { useEffect, useMemo, useState } from "react";
import { useNavigate } from "react-router-dom";
import { api, type UiEntity } from "../../api";

type Hit = { entity: string; slug: string; id: string; label: string; snippet: string };
type SearchGroup = { entity: string; label: string; hits: Hit[] };
type Command = { id: string; group: string; label: string; hint?: string; run: () => void };

const RECENT_KEY = "qefro_recent_searches";

function readRecent(): string[] {
  try {
    const raw = JSON.parse(localStorage.getItem(RECENT_KEY) || "[]");
    return Array.isArray(raw) ? raw.filter((v) => typeof v === "string").slice(0, 8) : [];
  } catch {
    return [];
  }
}

function rememberSearch(q: string) {
  const next = [q, ...readRecent().filter((item) => item.toLowerCase() !== q.toLowerCase())].slice(0, 8);
  localStorage.setItem(RECENT_KEY, JSON.stringify(next));
}

export default function CommandPalette({
  entities,
  studio,
  open,
  onOpenChange,
}: {
  entities: UiEntity[];
  studio?: boolean;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const [q, setQ] = useState("");
  const [results, setResults] = useState<Hit[]>([]);
  const [groups, setGroups] = useState<SearchGroup[]>([]);
  const [reports, setReports] = useState<Array<{ name: string; label: string }>>([]);
  const [recent, setRecent] = useState<string[]>(() => readRecent());
  const [active, setActive] = useState(0);
  const navigate = useNavigate();

  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        onOpenChange(!open);
      }
      if (e.key === "Escape") onOpenChange(false);
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onOpenChange]);

  useEffect(() => {
    if (!open) {
      setQ("");
      setResults([]);
      setGroups([]);
      setActive(0);
      setRecent(readRecent());
      return;
    }
    api
      .reports()
      .then((d) => setReports((d.reports ?? []).map((r) => ({ name: r.name, label: r.label }))))
      .catch(() => setReports([]));
  }, [open]);

  useEffect(() => {
    if (!open || !q.trim()) {
      setResults([]);
      setGroups([]);
      return;
    }
    const handle = window.setTimeout(() => {
      api
        .search(q)
        .then((d) => {
          setResults(d.results);
          setGroups(d.groups ?? []);
        })
        .catch(() => {
          setResults([]);
          setGroups([]);
        });
    }, 150);
    return () => window.clearTimeout(handle);
  }, [open, q]);

  const standalone = useMemo(
    () => entities.filter((e) => e.standalone !== false && !e.singleton && !e.child_of),
    [entities],
  );

  const commands = useMemo<Command[]>(() => {
    const go = (to: string) => () => {
      navigate(to);
      onOpenChange(false);
    };
    const list: Command[] = [
      { id: "dash", group: "Go to", label: "Dashboard", run: go("/") },
      { id: "reports", group: "Go to", label: "Reports", run: go("/reports") },
      { id: "settings", group: "Go to", label: "Settings", run: go("/settings") },
    ];
    if (studio) list.push({ id: "studio", group: "Go to", label: "Studio", run: go("/studio") });
    for (const e of standalone) {
      list.push({
        id: `go-${e.slug}`,
        group: "Go to",
        label: `Go to ${e.label_plural}`,
        run: go(`/${e.slug}`),
      });
      list.push({
        id: `new-${e.slug}`,
        group: "Create",
        label: `Create ${e.label}`,
        run: go(`/${e.slug}/new`),
      });
    }
    for (const r of reports) {
      list.push({
        id: `report-${r.name}`,
        group: "Reports",
        label: `Run ${r.label}`,
        run: go("/reports"),
      });
    }
    const needle = q.trim().toLowerCase();
    return needle ? list.filter((c) => c.label.toLowerCase().includes(needle)) : list;
  }, [standalone, reports, q, navigate, onOpenChange, studio]);

  const groupedHits = useMemo(() => {
    if (groups.length > 0) {
      return groups.map((g) => [g.label || g.entity, g.hits] as [string, Hit[]]);
    }
    const map = new Map<string, Hit[]>();
    for (const hit of results) {
      const list = map.get(hit.entity) ?? [];
      list.push(hit);
      map.set(hit.entity, list);
    }
    return Array.from(map.entries());
  }, [results, groups]);

  const flat: Array<{ kind: "cmd"; cmd: Command } | { kind: "hit"; hit: Hit }> = [
    ...commands.map((cmd) => ({ kind: "cmd" as const, cmd })),
    ...results.map((hit) => ({ kind: "hit" as const, hit })),
  ];

  function runIndex(i: number) {
    const item = flat[i];
    if (!item) return;
    if (item.kind === "cmd") item.cmd.run();
    else {
      rememberSearch(q.trim());
      navigate(`/${item.hit.slug}/${item.hit.id}`);
      onOpenChange(false);
    }
  }

  if (!open) return null;

  return (
    <div className="palette-backdrop" onClick={() => onOpenChange(false)}>
      <div
        className="palette"
        role="dialog"
        aria-label="Command palette"
        onClick={(e) => e.stopPropagation()}
      >
        <input
          autoFocus
          placeholder="Create, go to, or search…"
          value={q}
          aria-label="Command or search"
          onChange={(e) => {
            setQ(e.target.value);
            setActive(0);
          }}
          onKeyDown={(e) => {
            if (e.key === "ArrowDown") {
              e.preventDefault();
              setActive((n) => Math.min(n + 1, Math.max(0, flat.length - 1)));
            }
            if (e.key === "ArrowUp") {
              e.preventDefault();
              setActive((n) => Math.max(n - 1, 0));
            }
            if (e.key === "Enter") {
              e.preventDefault();
              runIndex(active);
            }
          }}
        />
        <ul>
          {commands.map((c, i) => (
            <li key={c.id}>
              <button
                type="button"
                className={i === active ? "active ghost" : "ghost"}
                onMouseEnter={() => setActive(i)}
                onClick={() => c.run()}
              >
                <span className="muted">{c.group}</span> {c.label}
              </button>
            </li>
          ))}
          {groupedHits.map(([entity, hits]) => (
            <li key={entity} className="palette-group">
              <div className="palette-heading">{entity}</div>
              {hits.map((r) => {
                const idx = commands.length + results.indexOf(r);
                return (
                  <button
                    key={`${r.slug}-${r.id}`}
                    type="button"
                    className={idx === active ? "active ghost" : "ghost"}
                    onMouseEnter={() => setActive(idx)}
                    onClick={() => {
                      rememberSearch(q.trim());
                      navigate(`/${r.slug}/${r.id}`);
                      onOpenChange(false);
                    }}
                  >
                    <strong>{r.label}</strong>
                    {r.snippet ? <span className="muted"> · {r.snippet}</span> : null}
                  </button>
                );
              })}
            </li>
          ))}
          {!q && recent.length > 0 ? (
            <li className="palette-group">
              <div className="palette-heading">Recent searches</div>
              {recent.map((item) => (
                <button
                  key={item}
                  type="button"
                  className="ghost"
                  onClick={() => {
                    setQ(item);
                    setActive(0);
                  }}
                >
                  {item}
                </button>
              ))}
            </li>
          ) : null}
          {q && flat.length === 0 ? <li className="muted">No matches.</li> : null}
        </ul>
      </div>
    </div>
  );
}
