import type { ComponentType } from "react";
import type { UiEntity } from "../metadata/types";
import type { Widget } from "../metadata/registry";
import { registerWidget } from "../metadata/registry";
import { registerView, type CollectionView } from "../views/registry";

export type EntityComponentOverrides = {
  list?: ComponentType<Record<string, unknown>>;
  card?: ComponentType<Record<string, unknown>>;
  form?: ComponentType<Record<string, unknown>>;
  detail?: ComponentType<Record<string, unknown>>;
  header?: ComponentType<Record<string, unknown>>;
  field?: ComponentType<Record<string, unknown>>;
};

export type PageRegistration = {
  component: ComponentType<Record<string, unknown>>;
  /** Route inside the authenticated workspace. Defaults to `/pages/{name}`. */
  path?: string;
  label?: string;
  section?: string;
  /** When true, also add a navigation item. Default false. */
  nav?: boolean;
};

export type NavRegistration = {
  label: string;
  to: string;
  section?: string;
};

export type DashboardWidgetComponent = ComponentType<{
  card: Record<string, unknown>;
  slug?: string;
  currency: string;
  locale: string;
  onSegment?: (card: Record<string, unknown>, slug: string | undefined, label: string) => void;
}>;

export type ActionRegistration = {
  entity?: string;
  name: string;
  label: string;
  render?: ComponentType<Record<string, unknown>>;
};

export type ThemeRegistration = {
  name: string;
  primary?: string;
  accent?: string;
  radius?: ThemeRadius;
  density?: "comfortable" | "compact";
  mode?: "light" | "dark" | "system";
};

export type ThemeRadius = "small" | "medium" | "large" | string;

export type ThemeConfig = {
  primary?: string;
  accent?: string;
  secondary?: string;
  radius?: ThemeRadius;
  density?: "comfortable" | "compact";
  mode?: "light" | "dark" | "system";
  fontFamily?: string;
  logo?: string;
  favicon?: string;
};

export type ExtensionInput = {
  page?: { name: string } & PageRegistration;
  widget?: { name: string; component: DashboardWidgetComponent };
  field?: { name: string; component: Widget };
  entity?: { name: string } & EntityComponentOverrides;
  navigation?: NavRegistration;
  action?: ActionRegistration;
  dashboard?: { name: string; widget: DashboardWidgetComponent };
  theme?: ThemeRegistration;
};

function entityKey(name: string) {
  return name.trim().toLowerCase();
}

export class ExtensionRegistry {
  readonly pages = new Map<string, PageRegistration & { name: string }>();
  readonly entities = new Map<string, EntityComponentOverrides>();
  readonly dashboardWidgets = new Map<string, DashboardWidgetComponent>();
  readonly navigation: NavRegistration[] = [];
  readonly actions: ActionRegistration[] = [];
  readonly themes = new Map<string, ThemeRegistration>();
  fieldRenderers = new Map<string, ComponentType<Record<string, unknown>>>();

  page(name: string, def: PageRegistration) {
    this.pages.set(name, { ...def, name });
    if (def.nav) {
      this.navigation.push({
        label: def.label || name,
        to: def.path || `/pages/${name}`,
        section: def.section,
      });
    }
  }

  getPage(name: string | undefined | null) {
    if (!name) return undefined;
    return this.pages.get(name);
  }

  extendEntity(name: string, overrides: EntityComponentOverrides) {
    const key = entityKey(name);
    const prev = this.entities.get(key) ?? {};
    this.entities.set(key, { ...prev, ...overrides });
  }

  entityOverrides(entity: UiEntity | string | undefined | null): EntityComponentOverrides {
    if (!entity) return {};
    if (typeof entity === "string") {
      return this.entities.get(entityKey(entity)) ?? {};
    }
    return (
      this.entities.get(entityKey(entity.entity)) ??
      this.entities.get(entityKey(entity.slug)) ??
      this.entities.get(entityKey(entity.label)) ??
      {}
    );
  }

  registerField(name: string, widget: Widget) {
    registerWidget(name, widget);
  }

  registerView(name: string, view: CollectionView) {
    registerView(name, view);
  }

  registerDashboardWidget(name: string, component: DashboardWidgetComponent) {
    this.dashboardWidgets.set(name, component);
  }

  getDashboardWidget(name: string | undefined | null) {
    if (!name) return undefined;
    return this.dashboardWidgets.get(name);
  }

  addNavigation(item: NavRegistration) {
    this.navigation.push(item);
  }

  addAction(action: ActionRegistration) {
    this.actions.push(action);
  }

  registerTheme(theme: ThemeRegistration) {
    this.themes.set(theme.name, theme);
  }

  apply(ext: ExtensionInput) {
    if (ext.page) this.page(ext.page.name, ext.page);
    if (ext.widget) this.registerDashboardWidget(ext.widget.name, ext.widget.component);
    if (ext.field) this.registerField(ext.field.name, ext.field.component);
    if (ext.entity) this.extendEntity(ext.entity.name, ext.entity);
    if (ext.navigation) this.addNavigation(ext.navigation);
    if (ext.action) this.addAction(ext.action);
    if (ext.dashboard) this.registerDashboardWidget(ext.dashboard.name, ext.dashboard.widget);
    if (ext.theme) this.registerTheme(ext.theme);
  }

  reset() {
    this.pages.clear();
    this.entities.clear();
    this.dashboardWidgets.clear();
    this.navigation.length = 0;
    this.actions.length = 0;
    this.themes.clear();
    this.fieldRenderers.clear();
  }
}

export const defaultExtensions = new ExtensionRegistry();
