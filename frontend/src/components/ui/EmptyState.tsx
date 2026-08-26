import type { ReactNode } from "react";

export function EmptyState({
  title,
  description,
  action,
}: {
  title: string;
  description?: string;
  action?: ReactNode;
}) {
  return (
    <div className="empty-state" role="status">
      <h3>{title}</h3>
      {description ? <p className="muted">{description}</p> : null}
      {action ? <div className="empty-action">{action}</div> : null}
    </div>
  );
}

export function Skeleton({
  rows = 5,
  variant = "table",
}: {
  rows?: number;
  variant?: "table" | "cards" | "kanban" | "form" | "calendar";
}) {
  if (variant === "cards") {
    return (
      <div className="skeleton-cards" aria-busy="true" aria-live="polite">
        <span className="sr-only">Loading</span>
        {Array.from({ length: Math.max(rows, 3) }, (_, i) => (
          <div key={i} className="skeleton-card" />
        ))}
      </div>
    );
  }
  if (variant === "kanban") {
    return (
      <div className="kanban skeleton-kanban" aria-busy="true" aria-live="polite">
        <span className="sr-only">Loading</span>
        {Array.from({ length: 3 }, (_, i) => (
          <section key={i} className="kanban-col">
            <div className="skeleton-row" />
            <div className="skeleton-row" />
            <div className="skeleton-row" />
          </section>
        ))}
      </div>
    );
  }
  if (variant === "calendar") {
    return (
      <div className="cal-grid skeleton-calendar" aria-busy="true" aria-live="polite">
        <span className="sr-only">Loading</span>
        {Array.from({ length: 7 }, (_, i) => (
          <div key={i} className="cal-day">
            <div className="skeleton-row" />
            <div className="skeleton-row" />
          </div>
        ))}
      </div>
    );
  }
  return (
    <div className={`skeleton-table${variant === "form" ? " skeleton-form" : ""}`} aria-busy="true" aria-live="polite">
      <span className="sr-only">Loading</span>
      {Array.from({ length: rows }, (_, i) => (
        <div key={i} className="skeleton-row" />
      ))}
    </div>
  );
}

export function ErrorState({ message, onRetry }: { message: string; onRetry?: () => void }) {
  return (
    <div className="error-state">
      <p className="error" role="alert">
        {message}
      </p>
      {onRetry ? (
        <button type="button" className="ghost" onClick={onRetry}>
          Retry
        </button>
      ) : null}
    </div>
  );
}
