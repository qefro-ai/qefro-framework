import { drilldownPath, drilldownSearch } from "./dashboard";

describe("dashboard drilldown", () => {
  const now = new Date("2026-08-16T12:00:00");

  it("expands today into the supported between filter", () => {
    expect(drilldownSearch([{ field: "reservation_date", value: "today" }], now)).toBe(
      "reservation_date.between=2026-08-16%2C2026-08-16&reservation_date.preset=today",
    );
  });

  it("passes status through the existing eq filter", () => {
    expect(drilldownPath("reservations", [{ field: "status", value: "Pending" }], now)).toBe(
      "/reservations?status=Pending",
    );
  });
});
