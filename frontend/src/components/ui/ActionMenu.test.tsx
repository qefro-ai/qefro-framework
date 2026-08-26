import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ActionMenu } from "./ActionMenu";

describe("ActionMenu", () => {
  it("hides empty menus and respects hidden items", () => {
    const { container } = render(
      <ActionMenu items={[{ key: "x", label: "Hidden", hidden: true }]} />,
    );
    expect(container).toBeEmptyDOMElement();
  });

  it("opens, selects, and closes on Escape", async () => {
    const onSelect = vi.fn();
    render(
      <ActionMenu
        items={[
          { key: "export", label: "Export", onSelect },
          { key: "print", label: "Print", href: "/print", target: "_blank" },
        ]}
      />,
    );
    await userEvent.click(screen.getByRole("button", { name: "More" }));
    expect(screen.getByRole("menu")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("menuitem", { name: "Export" }));
    expect(onSelect).toHaveBeenCalled();
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "More" }));
    await userEvent.keyboard("{Escape}");
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
  });
});
