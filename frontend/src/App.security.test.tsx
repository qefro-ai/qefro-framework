import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import App from "./App";

describe("unauthenticated browser navigation", () => {
  beforeEach(() => {
    localStorage.clear();
  });

  afterEach(() => {
    vi.restoreAllMocks();
    localStorage.clear();
  });

  it.each(["/studio", "/admin", "/audit", "/settings", "/settings/audit", "/customers/other-id", "/pages/secret"])(
    "redirects %s to login without fetching app data",
    (path) => {
      const fetchSpy = vi.spyOn(globalThis, "fetch");
      render(
        <MemoryRouter initialEntries={[path]}>
          <App />
        </MemoryRouter>,
      );
      expect(screen.getByRole("heading", { name: "Welcome back" })).toBeInTheDocument();
      expect(screen.queryByText("Qefro Studio")).not.toBeInTheDocument();
      expect(screen.queryByText("Audit log")).not.toBeInTheDocument();
      expect(fetchSpy).not.toHaveBeenCalled();
    },
  );
});
