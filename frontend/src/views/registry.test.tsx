import { render } from "@testing-library/react";
import { registerView, registeredViews, renderView } from "./registry";

describe("ViewRegistry", () => {
  it("registers custom views without modifying core renderers", () => {
    registerView("custom", () => <div>custom-board</div>);
    expect(registeredViews()).toContain("custom");
    const node = renderView("custom", {
      meta: {
        entity: "X",
        label: "X",
        label_plural: "Xs",
        slug: "xs",
        searchable: false,
        fields: [],
      },
      entities: [],
      slug: "xs",
      rows: [],
      total: 0,
      loading: false,
      onReload: () => undefined,
      onError: () => undefined,
    });
    const { getByText } = render(<>{node}</>);
    expect(getByText("custom-board")).toBeInTheDocument();
  });
});
