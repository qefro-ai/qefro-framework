export type EditorStep =
  | { kind: "wait"; wait: string }
  | {
      kind: "condition";
      field: string;
      equals: string;
      then: EditorStep[];
      else: EditorStep[];
    }
  | {
      kind: "action";
      actionKind: string;
      template?: string;
      role?: string;
      message?: string;
      transition?: string;
    };

export function validateAutomationGraph(input: {
  trigger?: string;
  steps: EditorStep[];
}): string[] {
  const errors: string[] = [];
  if (!input.trigger?.trim()) errors.push("Missing trigger");
  if (input.steps.length === 0) errors.push("Missing action");
  walk(input.steps, errors, new Set());
  return errors;
}

function walk(steps: EditorStep[], errors: string[], seen: Set<EditorStep>) {
  for (const step of steps) {
    if (seen.has(step)) {
      errors.push("Cycle detected");
      return;
    }
    seen.add(step);
    if (step.kind === "wait") {
      if (!step.wait.trim()) errors.push("Invalid wait");
    } else if (step.kind === "condition") {
      if (!step.field.trim()) errors.push("Invalid condition");
      walk(step.then, errors, seen);
      walk(step.else, errors, seen);
    } else {
      if (!step.actionKind.trim()) errors.push("Invalid action");
      if (step.actionKind === "send_communication" && !step.template?.trim()) {
        errors.push("Missing recipient");
      }
      if (step.actionKind === "transition" && !step.transition?.trim()) {
        errors.push("Invalid transition");
      }
    }
  }
}
