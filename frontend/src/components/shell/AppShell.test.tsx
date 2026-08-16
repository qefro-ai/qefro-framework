import type { ReactElement } from "react";
import { render, screen } from "@testing-library/react";
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
});
