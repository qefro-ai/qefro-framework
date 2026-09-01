# @qefro/js

`@qefro/js` is the Qefro UI runtime: design system, entity renderer, workspace, and extension API. The app in `frontend/` consumes it. Qefro Studio stays in the application.

It is **not** a security boundary and **not** a place for Estate/CRM/restaurant business UI. Generic metadata rendering is the default. Applications register pages, cards, and widgets when they need something extra.

```ts
import { Qefro } from "@qefro/js";
import "@qefro/js/styles.css";

const qefro = new Qefro({ apiUrl: "/api/v1" });
await qefro.init();

qefro.ui.list("Property");
qefro.ui.form("Lead");
qefro.ui.detail("Booking");
qefro.ui.dashboard("Sales");
qefro.ui.extend("Property", { card: PropertyCard });
qefro.page("property-map", { component: PropertyMap });
```

Package documentation: [`packages/qefro-js/README.md`](https://github.com/qefro-ai/qefro-framework/blob/main/packages/qefro-js/README.md).

Related: [UI](ui.md), [UI 2.1](ui-2.md), [QefroClient](sdk.md), [Theming](theming.md), [Components](ui-components.md).
