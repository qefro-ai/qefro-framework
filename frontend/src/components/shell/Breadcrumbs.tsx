import { Link, useLocation, useParams } from "react-router-dom";
import type { UiEntity } from "../../api";

export function Breadcrumbs({ entities }: { entities: UiEntity[] }) {
  const location = useLocation();
  const params = useParams();
  const parts = location.pathname.split("/").filter(Boolean);
  if (parts.length === 0) {
    return (
      <nav className="breadcrumbs" aria-label="Breadcrumb">
        <span aria-current="page">Dashboard</span>
      </nav>
    );
  }
  const crumbs: Array<{ to: string; label: string }> = [{ to: "/", label: "Home" }];
  if (parts[0] === "settings") crumbs.push({ to: "/settings", label: "Settings" });
  else if (parts[0] === "reports") crumbs.push({ to: "/reports", label: "Reports" });
  else if (parts[0] === "studio") crumbs.push({ to: "/studio", label: "Studio" });
  else {
    const meta = entities.find((e) => e.slug === parts[0]);
    if (meta) {
      crumbs.push({ to: `/${meta.slug}`, label: meta.label_plural });
      if (parts[1] === "new") crumbs.push({ to: `/${meta.slug}/new`, label: `New ${meta.label}` });
      else if (parts[1] && parts[2] === "edit") {
        crumbs.push({ to: `/${meta.slug}/${parts[1]}`, label: params.id || "Record" });
        crumbs.push({ to: location.pathname, label: "Edit" });
      } else if (parts[1]) crumbs.push({ to: location.pathname, label: params.id || "Record" });
    }
  }
  return (
    <nav className="breadcrumbs" aria-label="Breadcrumb">
      {crumbs.map((c, i) => {
        const last = i === crumbs.length - 1;
        return (
          <span key={c.to + c.label}>
            {i > 0 ? <span className="crumb-sep" aria-hidden="true">/</span> : null}
            {last ? (
              <span aria-current="page">{c.label}</span>
            ) : (
              <Link to={c.to}>{c.label}</Link>
            )}
          </span>
        );
      })}
    </nav>
  );
}
