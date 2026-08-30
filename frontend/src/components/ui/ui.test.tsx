import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { StatusBadge } from "./StatusBadge";
import { EmptyState, ErrorState, Skeleton } from "./EmptyState";
import { PageHeader } from "./PageHeader";
import { SectionHeader } from "./SectionHeader";

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

  it("offers retry on errors and card skeletons", async () => {
    const onRetry = vi.fn();
    render(
      <>
        <ErrorState message="Unable to load." onRetry={onRetry} />
        <Skeleton variant="cards" rows={3} />
      </>,
    );
    expect(screen.getAllByText("Loading").length).toBeGreaterThan(0);
    await userEvent.click(screen.getByRole("button", { name: "Retry" }));
    expect(onRetry).toHaveBeenCalled();
  });

  it("renders a consistent page header", () => {
    render(<PageHeader kicker="Note" title="Notes" actions={<button>New Note</button>} />);
    expect(screen.getByText("Note")).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Notes" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "New Note" })).toBeInTheDocument();
  });

  it("renders a section header", () => {
    render(<SectionHeader title="Details" actions={<a href="/x">View all</a>} />);
    expect(screen.getByRole("heading", { name: "Details" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "View all" })).toBeInTheDocument();
  });
});
