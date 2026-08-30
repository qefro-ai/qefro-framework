import { useState } from "react";
import type { EntityAction, WorkflowAction } from "../../sdk/client";
import { ActionMenu } from "../ui/ActionMenu";
import { ConfirmDialog } from "../ui/ConfirmDialog";

export function transitionNeedsConfirm(t: WorkflowAction) {
  return Boolean(t.requires_confirmation || t.confirmation || t.confirmation_message);
}

export function ActionBar({
  actions,
  transitions,
  onAction,
  onTransition,
  compact,
}: {
  actions: EntityAction[];
  transitions?: WorkflowAction[];
  onAction: (name: string, action: EntityAction) => void;
  onTransition?: (name: string) => void;
  compact?: boolean;
}) {
  const covered = new Set(
    actions.flatMap((a) => [a.name, a.workflow_transition].filter((v): v is string => Boolean(v))),
  );
  const extra = (transitions ?? []).filter(
    (t) => !covered.has(t.name) && !covered.has(t.to) && !covered.has(t.id || ""),
  );
  const primary = actions.filter((a) => a.style !== "danger").slice(0, compact ? 1 : 2);
  const rest = actions.filter((a) => !primary.includes(a));
  const shownTransitions = actions.length === 0 ? (transitions ?? []) : extra;
  const [pending, setPending] = useState<{ kind: "action" | "transition"; name: string; message: string; label: string } | null>(
    null,
  );

  function requestAction(action: EntityAction) {
    const message = action.confirmation_message || `${action.label || action.name}?`;
    if (action.requires_confirmation || action.confirmation_message) {
      setPending({ kind: "action", name: action.name, message, label: action.label || action.name });
      return;
    }
    onAction(action.name, action);
  }

  function requestTransition(t: WorkflowAction) {
    const message = t.confirmation_message || `Move to ${t.to}?`;
    if (transitionNeedsConfirm(t)) {
      setPending({ kind: "transition", name: t.name, message, label: t.label || t.name });
      return;
    }
    onTransition?.(t.name);
  }

  return (
    <div className={`actions action-bar${compact ? " is-compact" : ""}`}>
      {primary.map((action) => (
        <button
          key={action.name}
          type="button"
          className={action.style === "ghost" ? "ghost" : undefined}
          onClick={() => requestAction(action)}
        >
          {action.label || action.name}
        </button>
      ))}
      {shownTransitions.map((t) => (
        <button key={t.name} type="button" onClick={() => requestTransition(t)}>
          {t.label || t.name}
        </button>
      ))}
      <ActionMenu
        items={rest.map((action) => ({
          key: action.name,
          label: action.label || action.name,
          danger: action.style === "danger",
          onSelect: () => requestAction(action),
        }))}
      />
      <ConfirmDialog
        open={Boolean(pending)}
        title={pending?.label}
        message={pending?.message || ""}
        confirmLabel="Confirm"
        danger={pending?.kind === "transition" && /cancel/i.test(pending.name)}
        onCancel={() => setPending(null)}
        onConfirm={() => {
          if (!pending) return;
          const next = pending;
          setPending(null);
          if (next.kind === "action") {
            const action = actions.find((a) => a.name === next.name);
            if (action) onAction(action.name, action);
          } else {
            onTransition?.(next.name);
          }
        }}
      />
    </div>
  );
}
