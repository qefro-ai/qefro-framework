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
    <header className="page-header">
      {kicker ? <div className="badge">{kicker}</div> : null}
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
