import { loadPrefs, savePrefs, resolvedTheme } from "./prefs";

describe("prefs", () => {
  beforeEach(() => localStorage.clear());

  it("round-trips tenant and user scoped preferences", () => {
    savePrefs("tenant-a", "ada@example.com", {
      theme: "dark",
      density: "compact",
      sidebarCollapsed: true,
      tables: { customers: { pageSize: 50, columns: ["name"] } },
    });
    const prefs = loadPrefs("tenant-a", "ada@example.com");
    expect(prefs.theme).toBe("dark");
    expect(prefs.density).toBe("compact");
    expect(prefs.tables.customers.pageSize).toBe(50);
    expect(loadPrefs("tenant-b", "ada@example.com").theme).toBe("system");
  });

  it("stores the preferred view per entity", () => {
    savePrefs("t", "u", {
      theme: "light",
      density: "comfortable",
      sidebarCollapsed: false,
      tables: { reservations: { view: "kanban" } },
    });
    expect(loadPrefs("t", "u").tables.reservations.view).toBe("kanban");
  });

  it("resolves system theme", () => {
    expect(resolvedTheme("light")).toBe("light");
    expect(resolvedTheme("dark")).toBe("dark");
  });
});
