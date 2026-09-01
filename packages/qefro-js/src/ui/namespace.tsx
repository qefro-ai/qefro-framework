import { createElement, type ComponentType } from "react";
import type { Qefro } from "../core/runtime";
import type { EntityComponentOverrides } from "../core/extensions";
import type { ViewKind } from "../metadata/types";
import { Button } from "../components/ui/Button";
import { Card } from "../components/ui/Card";
import { ConfirmDialog } from "../components/ui/ConfirmDialog";
import { showSnackbar, type SnackbarTone } from "../components/ui/Snackbar";
import { Tabs } from "../components/ui/Tabs";
import { AppShell } from "../components/shell/AppShell";
import { EntityListRenderer, EntityFormRenderer, EntityDetailRenderer } from "../renderer";
import Dashboard from "../pages/Dashboard";

export type ListOptions = {
  view?: ViewKind | "table" | "cards" | "compact";
};

export class QefroUI {
  constructor(private readonly runtime: Qefro) {}

  button = Button;
  card = Card;
  dialog = ConfirmDialog;
  tabs = Tabs;

  table = (entity: string, options?: ListOptions) =>
    this.list(entity, { view: "table", ...options });

  toast(message: string, tone: SnackbarTone = "success") {
    return showSnackbar(message, tone);
  }

  notify(message: string, tone: SnackbarTone = "info") {
    return showSnackbar(message, tone);
  }

  extend(entity: string, overrides: EntityComponentOverrides) {
    this.runtime.extensions.extendEntity(entity, overrides);
    return this;
  }

  list(entity: string, options?: ListOptions) {
    const view = normalizeListView(options?.view);
    return createElement(EntityListRenderer, {
      entities: this.runtime.getEntities(),
      entity,
      view,
    });
  }

  form(entity: string) {
    return createElement(EntityFormRenderer, { entities: this.runtime.getEntities(), entity });
  }

  detail(entity: string) {
    return createElement(EntityDetailRenderer, { entities: this.runtime.getEntities(), entity });
  }

  dashboard(name?: string) {
    return createElement(Dashboard, {
      entities: this.runtime.getEntities(),
      config: this.runtime.getConfig(),
      dashboardName: name,
    });
  }

  workspace() {
    return AppShell as ComponentType<Record<string, unknown>>;
  }
}

function normalizeListView(view?: ListOptions["view"]): ViewKind | undefined {
  if (!view) return undefined;
  if (view === "table") return "list";
  if (view === "cards" || view === "compact") return "card";
  return view;
}
