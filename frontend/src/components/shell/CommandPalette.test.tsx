import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import CommandPalette from "./CommandPalette";
import type { UiEntity } from "../../api";

const customer: UiEntity = {
  entity: "Customer",
  label: "Customer",
  label_plural: "Customers",
  slug: "customers",
  searchable: true,
  fields: [],
  standalone: true,
};

describe("CommandPalette", () => {
  beforeEach(() => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async (input: RequestInfo) => {
        const url = String(input);
        const json = (body: unknown) =>
          Promise.resolve({ ok: true, status: 200, json: async () => body, statusText: "OK" });
        if (url.includes("/meta/reports")) return json({ reports: [{ name: "sales", label: "Today's Sales" }] });
        if (url.includes("/search")) {
          return json({
            results: [{ entity: "Customer", slug: "customers", id: "1", label: "Ahmed", snippet: "ahmed@example.com" }],
            groups: [
              {
                entity: "Customer",
                label: "Customers",
                hits: [{ entity: "Customer", slug: "customers", id: "1", label: "Ahmed", snippet: "ahmed@example.com" }],
              },
            ],
          });
        }
        return json({});
      }),
    );
  });

  it("lists create and go-to commands from metadata", async () => {
    render(
      <MemoryRouter>
        <CommandPalette entities={[customer]} open onOpenChange={() => undefined} />
      </MemoryRouter>,
    );
    expect(await screen.findByText(/Create Customer/)).toBeInTheDocument();
    expect(screen.getByText(/Go to Customers/)).toBeInTheDocument();
  });

  it("searches records", async () => {
    render(
      <MemoryRouter>
        <CommandPalette entities={[customer]} open onOpenChange={() => undefined} />
      </MemoryRouter>,
    );
    await userEvent.type(screen.getByLabelText("Command or search"), "Ahmed");
    expect(await screen.findByText("Ahmed")).toBeInTheDocument();
    expect(screen.getByText("Customers")).toBeInTheDocument();
  });

  it("shows recent searches", async () => {
    localStorage.setItem("qefro_recent_searches", JSON.stringify(["Ahmed"]));
    render(
      <MemoryRouter>
        <CommandPalette entities={[customer]} open onOpenChange={() => undefined} />
      </MemoryRouter>,
    );
    expect(await screen.findByText("Recent searches")).toBeInTheDocument();
    expect(screen.getByText("Ahmed")).toBeInTheDocument();
  });
});
