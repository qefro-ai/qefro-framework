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
        <ViewSelector views={["list", "kanban", "calendar"]} current="kanban" onChange={onChange} />
      </MemoryRouter>,
    );
    expect(screen.getByRole("tab", { name: "Kanban" })).toHaveAttribute("aria-selected", "true");
    screen.getByRole("tab", { name: "Calendar" }).click();
    expect(onChange).toHaveBeenCalledWith("calendar");
  });

  it("builds a deep-link that preserves filters", () => {
    const params = new URLSearchParams("status=Pending&search=ada");
    expect(viewSearchLink("reservations", "kanban", params)).toBe(
      "/reservations?status=Pending&search=ada&view=kanban",
    );
  });
});
