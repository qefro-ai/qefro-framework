import type { ReactElement } from "react";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { PrefsProvider } from "../../prefsContext";
import { AppShell } from "./AppShell";
import type { UiEntity } from "../../api";

const customer: UiEntity = {
  entity: "Customer",
  label: "Customer",
  label_plural: "Customers",
  slug: "customers",
  searchable: true,
  module: "crm",
  fields: [],
  standalone: true,
};

function wrap(ui: ReactElement) {
  return render(
    <MemoryRouter>
      <PrefsProvider tenantId="t" userId="ada">
        {ui}
      </PrefsProvider>
    </MemoryRouter>,
  );
}

describe("AppShell", () => {
  it("renders metadata navigation and top bar", () => {
    wrap(
      <AppShell
        appName="Demo"
        navEntities={[customer]}
        studio={false}
        userName="Ada"
        userEmail="ada@example.com"
        roles={["Admin"]}
      >
        <div>Content</div>
      </AppShell>,
    );
    expect(screen.getByText("Demo")).toBeInTheDocument();
    expect(screen.getByText("Customers")).toBeInTheDocument();
    expect(screen.getByText("Content")).toBeInTheDocument();
    expect(screen.getByText(/Search/)).toBeInTheDocument();
  });

  it("groups workspace navigation by section", () => {
    wrap(
      <AppShell
        appName="Kitchen"
        navEntities={[customer]}
        workspaceNav={[
          { label: "Orders", entity: "Order", slug: "orders", section: "Operations" },
          { label: "Customers", entity: "Customer", slug: "customers", section: "Catalog" },
        ]}
        studio={false}
        userName="Ada"
        userEmail="ada@example.com"
        roles={["Admin"]}
      >
        <div>Content</div>
      </AppShell>,
    );
    expect(screen.getByText("Operations")).toBeInTheDocument();
    expect(screen.getByText("Catalog")).toBeInTheDocument();
    expect(screen.getByText("Orders")).toBeInTheDocument();
  });

  it("opens a compact navigation drawer on mobile", async () => {
    const original = window.matchMedia;
    window.matchMedia = ((query: string) =>
      ({
        matches: String(query).includes("840"),
        media: query,
        onchange: null,
        addEventListener: () => undefined,
        removeEventListener: () => undefined,
        addListener: () => undefined,
        removeListener: () => undefined,
        dispatchEvent: () => false,
      })) as typeof window.matchMedia;
    wrap(
      <AppShell
        appName="Kitchen"
        navEntities={[customer]}
        workspaceNav={[
          { label: "Orders", entity: "Order", slug: "orders", section: "Operations" },
          { label: "Customers", entity: "Customer", slug: "customers", section: "Catalog" },
        ]}
        studio={false}
        userName="Ada"
        userEmail="ada@example.com"
        roles={["Admin"]}
      >
        <div>Content</div>
      </AppShell>,
    );
    expect(screen.getByRole("button", { name: "Open navigation" })).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Open navigation" }));
    expect(screen.getByRole("button", { name: "Close navigation" })).toBeInTheDocument();
    expect(screen.getByText("Operations")).toBeInTheDocument();
    expect(screen.getByText("Catalog")).toBeInTheDocument();
    window.matchMedia = original;
  });
});
