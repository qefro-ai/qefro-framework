import { describe, expect, it } from "vitest";
import { localToUtcIso, utcToLocalParts } from "../metadata/timezone";
import { fieldVisible } from "../metadata/conditions";

describe("timezone conversion", () => {
  it("displays Kolkata local from UTC", () => {
    const parts = utcToLocalParts("2026-08-15T14:30:00.000Z", "Asia/Kolkata");
    expect(parts.date).toBe("2026-08-15");
    expect(parts.time).toBe("20:00");
  });

  it("converts naive local back to UTC", () => {
    const iso = localToUtcIso("2026-08-15", "20:00", "Asia/Kolkata");
    expect(iso).toBe("2026-08-15T14:30:00.000Z");
  });
});

describe("conditional visibility", () => {
  it("shows cancellation reason only when cancelled", () => {
    const field = { visible_when: { field: "status", equals: "Cancelled" } };
    expect(fieldVisible(field, { status: "Cancelled" })).toBe(true);
    expect(fieldVisible(field, { status: "Pending" })).toBe(false);
  });
});
