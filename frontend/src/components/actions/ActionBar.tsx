import type { EntityAction, WorkflowAction } from "../../api";
import { ActionMenu } from "../ui/ActionMenu";

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
  const primary = actions.filter((a) => a.style !== "danger").slice(0, 2);
  const rest = actions.filter((a) => !primary.includes(a));
  const fallback = actions.length === 0 ? (transitions ?? []) : [];

  return (
    <div className="actions action-bar">
      {primary.map((action) => (
        <button
          key={action.name}
          type="button"
          className={action.style === "ghost" ? "ghost" : undefined}
          onClick={() => onAction(action.name, action)}
        >
          {action.label || action.name}
        </button>
      ))}
      {fallback.map((t) => (
        <button key={t.name} type="button" onClick={() => onTransition?.(t.name)}>
          {t.label || t.name}
        </button>
      ))}
      <ActionMenu
        items={rest.map((action) => ({
          key: action.name,
          label: action.label || action.name,
          danger: action.style === "danger",
          onSelect: () => onAction(action.name, action),
        }))}
      />
    </div>
  );
}
