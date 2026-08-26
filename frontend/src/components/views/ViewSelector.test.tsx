import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { ViewSelector, viewSearchLink } from "./ViewSelector";

describe("ViewSelector", () => {
  it("hides when only list is available", () => {
    const { container } = render(<ViewSelector views={["list"]} current="list" onChange={() => undefined} />);
    expect(container).toBeEmptyDOMElement();
  });

  it("renders available views and reports the selection", () => {
    const onChange = vi.fn();
    render(
      <MemoryRouter>
        <ViewSelector views={["list", "card", "kanban", "calendar"]} current="kanban" onChange={onChange} />
      </MemoryRouter>,
    );
    expect(screen.getByRole("tab", { name: "Kanban" })).toHaveAttribute("aria-selected", "true");
    expect(screen.getByRole("tab", { name: "Kanban" })).toHaveClass("is-active");
    expect(screen.getByRole("tab", { name: "Cards" })).toBeInTheDocument();
    screen.getByRole("tab", { name: "Calendar" }).click();
    expect(onChange).toHaveBeenCalledWith("calendar");
  });

  it("shows Cards only when that view is in the list", () => {
    render(
      <MemoryRouter>
        <ViewSelector views={["list", "kanban"]} current="list" onChange={() => undefined} />
      </MemoryRouter>,
    );
    expect(screen.queryByRole("tab", { name: "Cards" })).not.toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "List" })).toBeInTheDocument();
  });

  it("builds a deep-link that preserves filters", () => {
    const params = new URLSearchParams("status=Pending&search=ada");
    expect(viewSearchLink("reservations", "kanban", params)).toBe(
      "/reservations?status=Pending&search=ada&view=kanban",
    );
  });
});
