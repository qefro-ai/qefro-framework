import { datePresetRange, fileSize, relativeTime, statusTone } from "./format";

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
});
