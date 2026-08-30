import { render, screen } from "@testing-library/react";
import { Chart } from "./Chart";

describe("Chart", () => {
  const series = [
    { label: "Lead", value: 10 },
    { label: "Won", value: 40 },
  ];

  it("renders bar, area, and donut without entity branches", () => {
    const { rerender } = render(<Chart kind="bar" series={series} />);
    expect(screen.getByRole("img")).toBeInTheDocument();
    rerender(<Chart kind="area" series={series} />);
    expect(screen.getByRole("img")).toBeInTheDocument();
    rerender(<Chart kind="donut" series={series} />);
    expect(screen.getByRole("img")).toBeInTheDocument();
  });

  it("shows empty copy", () => {
    render(<Chart kind="bar" series={[]} />);
    expect(screen.getByText("No data")).toBeInTheDocument();
  });
});
