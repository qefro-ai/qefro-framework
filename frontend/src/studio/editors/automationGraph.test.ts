import { validateAutomationGraph, type EditorStep } from "./automationGraph";

describe("automation graph validation", () => {
  it("requires a trigger and steps", () => {
    const errors = validateAutomationGraph({ trigger: "", steps: [] });
    expect(errors).toContain("Missing trigger");
    expect(errors).toContain("Missing action");
  });

  it("accepts a linear wait and notify chain", () => {
    const steps: EditorStep[] = [
      { kind: "action", actionKind: "send_communication", template: "order_confirmed" },
      { kind: "wait", wait: "30m" },
      {
        kind: "condition",
        field: "status",
        equals: "Preparing",
        then: [{ kind: "action", actionKind: "notify", role: "Manager" }],
        else: [],
      },
    ];
    expect(validateAutomationGraph({ trigger: "order.confirmed", steps })).toEqual([]);
  });

  it("flags missing communication template and empty condition", () => {
    const steps: EditorStep[] = [
      { kind: "action", actionKind: "send_communication", template: "" },
      { kind: "condition", field: "", equals: "", then: [], else: [] },
    ];
    const errors = validateAutomationGraph({ trigger: "order.confirmed", steps });
    expect(errors).toContain("Missing recipient");
    expect(errors).toContain("Invalid condition");
  });
});
