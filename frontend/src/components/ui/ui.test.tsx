import { render, screen } from "@testing-library/react";
import { StatusBadge } from "./StatusBadge";
import { EmptyState, ErrorState, Skeleton } from "./EmptyState";

describe("StatusBadge", () => {
  it("uses metadata indicators", () => {
    render(<StatusBadge value="Pending" indicators={{ Pending: "warning" }} />);
    expect(screen.getByText("Pending").className).toMatch(/warning/);
  });
});

describe("empty loading error", () => {
  it("renders empty action", () => {
    render(<EmptyState title="No reservations yet" action={<button>New Reservation</button>} />);
    expect(screen.getByText("No reservations yet")).toBeInTheDocument();
    expect(screen.getByText("New Reservation")).toBeInTheDocument();
  });

  it("renders skeleton and error", () => {
    render(
      <>
        <Skeleton />
        <ErrorState message="You don't have permission to perform this action." />
      </>,
    );
    expect(screen.getByText("Loading")).toBeInTheDocument();
    expect(screen.getByRole("alert")).toHaveTextContent(/permission/i);
  });
});
