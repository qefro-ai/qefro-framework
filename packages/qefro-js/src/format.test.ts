import { datePresetRange, dueChip, fileSize, relativeTime, statusTone } from "./format";

describe("format", () => {
  it("computes date presets without sql", () => {
    const now = new Date(2026, 7, 16, 12, 0, 0);
    expect(datePresetRange("today", now)).toEqual({ from: "2026-08-16", to: "2026-08-16" });
    expect(datePresetRange("last_7_days", now)?.from).toBe("2026-08-10");
  });

  it("does not hardcode business statuses", () => {
    expect(statusTone("InventedState")).toBe("neutral");
  });

  it("formats relative time", () => {
    expect(relativeTime(new Date().toISOString())).toMatch(/second|now/i);
  });

  it("formats attachment sizes", () => {
    expect(fileSize(245 * 1024)).toMatch(/245 KB/);
    expect(fileSize(1.2 * 1024 * 1024)).toMatch(/1\.2 MB/);
  });

  it("derives due chips without persisting overdue", () => {
    const now = new Date(2026, 7, 30, 12, 0, 0);
    expect(dueChip(new Date(2026, 7, 29, 9, 0, 0).toISOString(), "Open", now)).toBe("Overdue");
    expect(dueChip(new Date(2026, 7, 30, 18, 0, 0).toISOString(), "Open", now)).toBe("Due today");
    expect(dueChip(new Date(2026, 7, 31, 9, 0, 0).toISOString(), "In Progress", now)).toBe(
      "Due tomorrow",
    );
    expect(dueChip(new Date(2026, 7, 29, 9, 0, 0).toISOString(), "Completed", now)).toBeNull();
    expect(dueChip(new Date(2026, 7, 29, 9, 0, 0).toISOString(), "Cancelled", now)).toBeNull();
  });
});
