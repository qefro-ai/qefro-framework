import type { ReactNode } from "react";

function foldLabel(value: string) {
  return value.trim().toLowerCase().replace(/[\s_-]+/g, "");
}

function isRedundantKicker(kicker: string, title: ReactNode) {
  if (typeof title !== "string") return false;
  const k = foldLabel(kicker);
  const t = foldLabel(title);
  if (!k || !t) return false;
  if (t === k) return true;
  if (t === `${k}s` || t === `${k}es`) return true;
  if (k.endsWith("y") && t === `${k.slice(0, -1)}ies`) return true;
  return false;
}

export function PageHeader({
  kicker,
  title,
  description,
  actions,
}: {
  kicker?: string;
  title: ReactNode;
  description?: ReactNode;
  actions?: ReactNode;
}) {
  const showKicker = Boolean(kicker?.trim()) && !isRedundantKicker(kicker!, title);
  return (
    <header className="page-header">
      {showKicker ? <div className="badge">{kicker}</div> : null}
      <div className="page-header-row doc-header">
        <div className="page-header-copy">
          <h2>{title}</h2>
          {description ? <div className="muted doc-meta">{description}</div> : null}
        </div>
        {actions ? <div className="actions page-header-actions">{actions}</div> : null}
      </div>
    </header>
  );
}
