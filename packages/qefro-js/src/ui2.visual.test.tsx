import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { TenantThemeContext } from "./metadata/context";
import { PrefsProvider } from "./prefsContext";
import { applyChrome } from "./prefs";
import { StatusBadge } from "./components/ui/StatusBadge";
import { EmptyState, ErrorState, Skeleton } from "./components/ui/EmptyState";
import { AppShell } from "./components/shell/AppShell";
import type { UiEntity } from "./api";

const entity: UiEntity = {
  entity: "Customer",
  label: "Customer",
  label_plural: "Customers",
  slug: "customers",
  searchable: true,
  fields: [],
};

describe("UI 2.0 visual states", () => {
  it("covers loading empty error compact dark", () => {
    applyChrome({ theme: "dark", density: "compact", sidebarCollapsed: false, tables: {} });
    expect(document.documentElement.dataset.theme).toBe("dark");
    expect(document.documentElement.dataset.density).toBe("compact");

    const { rerender } = render(
      <TenantThemeContext.Provider value={{ timezone: "UTC", locale: "en", currency: "USD" }}>
        <Skeleton />
      </TenantThemeContext.Provider>,
    );
    expect(screen.getByText("Loading")).toBeInTheDocument();

    rerender(<EmptyState title="No reservations yet" />);
    expect(screen.getByText("No reservations yet")).toBeInTheDocument();

    rerender(<ErrorState message="You don't have permission to perform this action." />);
    expect(screen.getByRole("alert")).toHaveTextContent(/permission/);

    rerender(<StatusBadge value="Confirmed" indicators={{ Confirmed: "info" }} />);
    expect(screen.getByText("Confirmed")).toBeInTheDocument();
  });

  it("renders shell on a constrained layout", () => {
    render(
      <MemoryRouter>
        <PrefsProvider tenantId="t" userId="u">
          <AppShell appName="Qefro" navEntities={[entity]} studio={false} userName="Ada" userEmail="ada@x.com" roles={[]}>
            <div>workspace</div>
          </AppShell>
        </PrefsProvider>
      </MemoryRouter>,
    );
    expect(screen.getByText("workspace")).toBeInTheDocument();
  });
});
