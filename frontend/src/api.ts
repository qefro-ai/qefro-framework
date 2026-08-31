/**
 * Compatibility facade. New code should import from `./sdk/client`.
 * `api` is a `QefroClient` instance — the same object, not a second client.
 */
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
  onAuthChange,
  listVisible,
  formVisible,
  detailVisible,
  expandedLabel,
  METADATA_EVENT,
} from "./sdk/client";
export type {
  UiEntity,
  UiField,
  WidgetOptions,
  UiWhen,
  TenantConfig,
  WorkflowAction,
  EntityAction,
  FieldError,
  Expanded,
} from "./sdk/client";
