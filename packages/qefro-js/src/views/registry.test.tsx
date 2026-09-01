import { useState } from "react";
import { render } from "@testing-library/react";
import { registerView, registeredViews, renderView } from "./registry";
import type { CollectionViewProps } from "./registry";

const props: CollectionViewProps = {
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
};

function HookyView(_props: CollectionViewProps) {
  const [n] = useState(1);
  return <div>hooky-{n}</div>;
}

describe("ViewRegistry", () => {
  it("registers custom views without modifying core renderers", () => {
    registerView("custom", () => <div>custom-board</div>);
    expect(registeredViews()).toContain("custom");
    const node = renderView("custom", props);
    const { getByText } = render(<>{node}</>);
    expect(getByText("custom-board")).toBeInTheDocument();
  });

  it("mounts views as components so hooks stay valid when switching", () => {
    registerView("plain", () => <div>plain</div>);
    registerView("hooky", HookyView);
    function Host({ view }: { view: string }) {
      return <>{renderView(view, props)}</>;
    }
    const { getByText, rerender } = render(<Host view="plain" />);
    expect(getByText("plain")).toBeInTheDocument();
    rerender(<Host view="hooky" />);
    expect(getByText("hooky-1")).toBeInTheDocument();
  });
});
