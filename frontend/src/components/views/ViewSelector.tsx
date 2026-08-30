import { NavLink } from "react-router-dom";
import type { ViewKind } from "../../metadata/types";

const LABELS: Record<ViewKind, string> = {
  list: "List",
  card: "Cards",
  kanban: "Kanban",
  calendar: "Calendar",
  chart: "Chart",
};

export function ViewSelector({
  views,
  current,
  onChange,
}: {
  views: ViewKind[];
  current: ViewKind;
  onChange: (view: ViewKind) => void;
}) {
  if (views.length <= 1) return null;
  return (
    <div className="view-selector view-selector-compact" role="tablist" aria-label="Views">
      {views.map((view) => (
        <button
          key={view}
          type="button"
          role="tab"
          aria-selected={current === view}
          className={current === view ? "is-active" : undefined}
          onClick={() => onChange(view)}
        >
          {LABELS[view] || view}
        </button>
      ))}
    </div>
  );
}

export function viewSearchLink(slug: string, view: ViewKind, params: URLSearchParams) {
  const next = new URLSearchParams(params);
  next.set("view", view);
  return `/${slug}?${next.toString()}`;
}

export function ViewLink({ to, children }: { to: string; children: string }) {
  return <NavLink to={to}>{children}</NavLink>;
}
