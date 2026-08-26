import type { ReactNode } from "react";

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
  return (
    <header className="page-header doc-header">
      <div>
        {kicker ? <div className="badge">{kicker}</div> : null}
        <h2>{title}</h2>
        {description ? <div className="muted doc-meta">{description}</div> : null}
      </div>
      {actions ? <div className="actions">{actions}</div> : null}
    </header>
  );
}
