import { useState } from "react";
import type { EntityAction, WorkflowAction } from "../../api";

export function ActionBar({
  actions,
  transitions,
  onAction,
  onTransition,
}: {
  actions: EntityAction[];
  transitions?: WorkflowAction[];
  onAction: (name: string, action: EntityAction) => void;
  onTransition?: (name: string) => void;
}) {
  const [open, setOpen] = useState(false);
  const primary = actions.filter((a) => a.style !== "danger").slice(0, 2);
  const rest = actions.filter((a) => !primary.includes(a));
  const fallback = actions.length === 0 ? transitions ?? [] : [];

  return (
    <div className="actions action-bar">
      {primary.map((action) => (
        <button
          key={action.name}
          className={action.style === "ghost" ? "ghost" : undefined}
          onClick={() => onAction(action.name, action)}
        >
          {action.label || action.name}
        </button>
      ))}
      {fallback.map((t) => (
        <button key={t.name} onClick={() => onTransition?.(t.name)}>
          {t.label || t.name}
        </button>
      ))}
      {rest.length > 0 ? (
        <div className="more-menu">
          <button type="button" className="ghost" aria-expanded={open} onClick={() => setOpen((v) => !v)}>
            More
          </button>
          {open ? (
            <ul role="menu">
              {rest.map((action) => (
                <li key={action.name} role="none">
                  <button
                    type="button"
                    role="menuitem"
                    className={action.style === "danger" ? "danger" : "ghost"}
                    onClick={() => {
                      setOpen(false);
                      onAction(action.name, action);
                    }}
                  >
                    {action.label || action.name}
                  </button>
                </li>
              ))}
            </ul>
          ) : null}
        </div>
      ) : null}
    </div>
  );
}
