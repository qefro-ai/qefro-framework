export type WidgetOptions = {
  currency?: string;
  precision?: number;
  timezone?: string;
  min?: unknown;
  max?: unknown;
  step?: number;
  hour12?: boolean;
  minute_step?: number;
  max_selections?: number;
  accept?: string[];
  max_size?: number;
  display_field?: string;
  search_fields?: string[];
  entity?: string;
  collapsed?: boolean;
  allow_create?: boolean;
  indicators?: Record<string, string>;
  columns?: number;
  editable?: boolean;
  addable?: boolean;
  deletable?: boolean;
  reorderable?: boolean;
};

export type UiWhen = {
  field: string;
  equals: unknown;
};

export type UiField = {
  name: string;
  type: string;
  label: string;
  description?: string;
  required: boolean;
  list: boolean;
  list_visible?: boolean;
  form: boolean;
  form_visible?: boolean;
  detail?: boolean;
  detail_visible?: boolean;
  filter: boolean;
  filterable?: boolean;
  searchable: boolean;
  sortable?: boolean;
  hidden?: boolean;
  disabled?: boolean;
  widget: string;
  widget_options?: WidgetOptions;
  placeholder?: string;
  help?: string;
  help_text?: string;
  section?: string;
  tab?: string;
  width?: string;
  order?: number;
  enum_values?: string[];
  relation?: string;
  relation_kind?: string;
  inverse_field?: string;
  readonly: boolean;
  visible_when?: UiWhen;
  readonly_when?: UiWhen;
  default_from?: string;
  computed?: boolean;
  formula?: string;
  permission_level?: number;
  allow_on_submit?: boolean;
  secret?: boolean;
  child_entity?: string;
};

export type UiEntity = {
  schema_version?: string;
  entity: string;
  label: string;
  label_plural: string;
  slug: string;
  searchable: boolean;
  workflow?: string;
  display_field?: string;
  module?: string;
  icon?: string;
  description?: string;
  fields: UiField[];
  tabs?: string[];
  sections?: string[];
  standalone?: boolean;
  child_of?: string;
  document?: {
    submit_enabled?: boolean;
    cancel_enabled?: boolean;
    duplicate_enabled?: boolean;
    lock_states?: string[];
  };
  naming?: { pattern: string; field?: string };
  singleton?: boolean;
  attachments?: boolean;
  capabilities?: EntityCapabilities;
  actions?: Array<{ name: string; label?: string; confirmation?: { required?: boolean; message?: string } }>;
  links?: Array<{ label: string; entity: string; relation: string }>;
  public_form?: { enabled?: boolean; slug?: string; fields?: string[] };
  list?: {
    columns?: Array<{ field: string; width?: number | string; widget?: string }>;
    default_sort?: { field: string; direction?: "asc" | "desc" };
    page_size?: number;
    group_by?: string;
  };
  views?: EntityViews;
  permissions?: EntityPermissions;
};

export type EntityPermissions = {
  list?: boolean;
  create?: boolean;
  read?: boolean;
  update?: boolean;
  delete?: boolean;
};

export type EntityCapabilities = {
  workflow?: boolean;
  activity?: boolean;
  comments?: boolean;
  attachments?: boolean;
  audit?: boolean;
  relations?: boolean;
  actions?: boolean;
};

export type RecordPermissions = {
  update?: boolean;
  delete?: boolean;
};

export type ViewKind = "list" | "card" | "kanban" | "calendar";

export type EntityViews = {
  list?: UiEntity["list"] & { group_by?: string };
  form?: { sections?: ViewSection[] };
  detail?: { sections?: ViewSection[] };
  card?: {
    enabled?: boolean;
    title?: string;
    subtitle?: string;
    image?: string;
    fields?: string[];
  };
  kanban?: {
    enabled?: boolean;
    group_by?: string;
    card?: { title?: string; subtitle?: string; fields?: string[] };
  };
  calendar?: {
    enabled?: boolean;
    start?: string;
    end?: string;
    time?: string;
    title?: string;
    subtitle?: string;
  };
};

export type ViewSection = {
  title: string;
  fields?: string[];
  visible_when?: UiWhen;
};

export type TenantTheme = {
  timezone: string;
  locale: string;
  currency: string;
  hour12?: boolean;
};
