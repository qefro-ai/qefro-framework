import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { StatusBadge } from "./StatusBadge";
import { EmptyState, ErrorState, Skeleton } from "./EmptyState";
import { PageHeader } from "./PageHeader";
import { SectionHeader } from "./SectionHeader";
import { SnackbarHost, showSnackbar } from "./Snackbar";
import { ConfirmDialog } from "./ConfirmDialog";
import { buttonClass } from "../../theme/tokens";

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
    render(<PageHeader kicker="Overview" title="Today" actions={<button>New Note</button>} />);
    expect(screen.getByText("Overview")).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Today" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "New Note" })).toBeInTheDocument();
  });

  it("renders a section header", () => {
    render(<SectionHeader title="Details" actions={<a href="/x">View all</a>} />);
    expect(screen.getByRole("heading", { name: "Details" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "View all" })).toBeInTheDocument();
  });

  it("hides a kicker that only repeats the title", () => {
    render(<PageHeader kicker="Customer" title="Customers" />);
    expect(screen.queryByText("Customer")).not.toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Customers" })).toBeInTheDocument();
  });
});

describe("M3 buttons and snackbar", () => {
  it("renders filled, tonal, outlined, text, icon, and destructive buttons", () => {
    render(
      <>
        <button className={buttonClass("filled")}>Save</button>
        <button className={buttonClass("tonal")}>Tonal</button>
        <button className={buttonClass("outlined")}>Outlined</button>
        <button className={buttonClass("text")}>Text</button>
        <button className={buttonClass("icon")} aria-label="More">
          ⋮
        </button>
        <button className={buttonClass("destructive")}>Delete</button>
      </>,
    );
    expect(screen.getByRole("button", { name: "Save" })).toHaveClass("btn");
    expect(screen.getByRole("button", { name: "Tonal" })).toHaveClass("tonal");
    expect(screen.getByRole("button", { name: "Outlined" })).toHaveClass("ghost");
    expect(screen.getByRole("button", { name: "Text" })).toHaveClass("text");
    expect(screen.getByRole("button", { name: "More" })).toHaveClass("icon-btn");
    expect(screen.getByRole("button", { name: "Delete" })).toHaveClass("danger");
  });

  it("announces snackbar messages", async () => {
    render(<SnackbarHost />);
    await act(async () => {
      showSnackbar("Saved");
    });
    expect(await screen.findByRole("status")).toHaveTextContent("Saved");
    await userEvent.click(screen.getByRole("button", { name: "Dismiss" }));
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
  });
});

describe("ConfirmDialog", () => {
  it("renders in-app copy and does not call confirm until accepted", async () => {
    const onConfirm = vi.fn();
    const onCancel = vi.fn();
    render(
      <ConfirmDialog
        open
        title="Archive 1 customer?"
        message="Archived records leave this list."
        confirmLabel="Archive"
        onConfirm={onConfirm}
        onCancel={onCancel}
      />,
    );
    expect(screen.getByRole("dialog", { name: "Archive 1 customer?" })).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(onConfirm).not.toHaveBeenCalled();
    expect(onCancel).toHaveBeenCalled();
  });
});
