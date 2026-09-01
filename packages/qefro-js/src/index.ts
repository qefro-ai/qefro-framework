/**
 * @qefro/js — Qefro UI runtime.
 *
 * Translates metadata + extensions + theme + permissions + runtime state
 * into application UI. Not a security boundary: EntityService remains
 * authoritative for every mutation.
 */
import "./widgets";

export { Qefro, getQefro } from "./core/runtime";
export type { QefroOptions, EntityHandle, ListOptions } from "./core/runtime";
export { QefroUI } from "./ui/namespace";
export {
  emitUiEvent,
  onUiEvent,
  resetUiEvents,
} from "./core/events";
export type { UiEventMap, UiEventName, UiEventHandler } from "./core/events";
export {
  defaultExtensions,
  ExtensionRegistry,
} from "./core/extensions";
export type {
  EntityComponentOverrides,
  PageRegistration,
  NavRegistration,
  ExtensionInput,
  ThemeConfig,
  ThemeRadius,
  DashboardWidgetComponent,
  ActionRegistration,
  ThemeRegistration,
} from "./core/extensions";
export {
  QefroProvider,
  useQefro,
  useQefroOptional,
  useQefroSnapshot,
  QefroRuntimeContext,
} from "./core/context";
export type { QefroSnapshot, QefroRuntimeValue } from "./core/context";

export {
  api,
  QefroClient,
  ApiError,
  ValidationError,
  tokenHeader,
  notifyMetadata,
  saveToken,
  clearToken,
  hasToken,
  TOKEN_KEY,
  onAuthChange,
  listVisible,
  formVisible,
  detailVisible,
  expandedLabel,
  METADATA_EVENT,
  setApiBaseUrl,
  getApiBaseUrl,
  resolveApiUrl,
} from "./sdk/client";
export type {
  TenantConfig,
  WorkflowAction,
  EntityAction,
  FieldError,
  Expanded,
} from "./sdk/client";

export type {
  UiEntity,
  UiField,
  WidgetOptions,
  UiWhen,
  WorkspaceNavItem,
  WorkspaceShortcut,
  PageDef,
  ViewKind,
  TenantTheme,
  EntityPermissions,
  EntityViews,
} from "./metadata/types";

export { TenantThemeContext, useTenantTheme } from "./metadata/context";
export {
  primaryNavEntities,
  settingsEntities,
  workspaceItemHref,
  isNavCandidate,
  resolveEntity,
} from "./metadata/navigation";
export {
  availableViews,
  defaultView,
  canCreate,
  canDelete,
  canUpdate,
  canExport,
  canDeleteRecord,
  canUpdateRecord,
  displayValue,
  listViewSpec,
} from "./metadata/views";
export { previewFormula } from "./metadata/formula";
export { registerWidget, renderWidget, registeredWidgets } from "./metadata/registry";
export type { WidgetProps, Widget } from "./metadata/registry";
export { registerView, renderView, registeredViews } from "./views/registry";
export type { CollectionView, CollectionViewProps } from "./views/registry";

export { applyTheme, applyBranding } from "./theme/apply";
export {
  QEFRO_COLOR_ROLES,
  MD_COLOR_ROLES,
  BUTTON_VARIANTS,
  buttonClass,
} from "./theme/tokens";
export type { ButtonVariant } from "./theme/tokens";

export { PrefsProvider, usePrefs, usePrefsOptional } from "./prefsContext";
export { applyChrome } from "./prefs";
export { showSnackbar, dismissSnackbar, SnackbarHost } from "./components/ui/Snackbar";
export type { SnackbarTone } from "./components/ui/Snackbar";

export { Button } from "./components/ui/Button";
export { Card } from "./components/ui/Card";
export { Chip } from "./components/ui/Chip";
export { ConfirmDialog } from "./components/ui/ConfirmDialog";
export { Tabs } from "./components/ui/Tabs";
export { PageHeader } from "./components/ui/PageHeader";
export { SectionHeader } from "./components/ui/SectionHeader";
export { EmptyState, ErrorState, Skeleton } from "./components/ui/EmptyState";
export { StatusBadge } from "./components/ui/StatusBadge";
export { ActionMenu } from "./components/ui/ActionMenu";
export { FormLayout } from "./components/forms/FormLayout";
export { default as AutomationRuns } from "./components/automation/AutomationRuns";
export { AppShell } from "./components/shell/AppShell";
export { EntityCard } from "./components/views/EntityCard";
export { FieldValue } from "./components/fields/FieldValue";
export { ActionBar } from "./components/actions/ActionBar";
export { FilterBar } from "./components/filters/FilterBar";
export { Chart } from "./components/dashboards/Chart";

export { default as EntityList } from "./pages/EntityList";
export { default as EntityForm } from "./pages/EntityForm";
export { default as EntityDetail } from "./pages/EntityDetail";
export { default as Dashboard, DashboardWidget } from "./pages/Dashboard";
export type { DashboardWidgetCard } from "./pages/Dashboard";
export { default as ComposedPage } from "./pages/ComposedPage";
export { default as Settings } from "./pages/Settings";
export { default as AuditLog } from "./pages/AuditLog";
export { default as Reports } from "./pages/Reports";
export { default as Login } from "./pages/Login";
export { default as PublicForm } from "./pages/PublicForm";

export { QefroRoutes, QefroPublicRoutes, RouteChangeListener } from "./router";
export {
  EntityListRenderer,
  EntityFormRenderer,
  EntityDetailRenderer,
  renderEntityCard,
  renderEntityHeader,
  renderFieldValue,
  renderDashboardWidget,
} from "./renderer";

export { friendlyError } from "./friendlyError";
export { t } from "./i18n";
