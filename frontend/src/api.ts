const TOKEN_KEY = "qefro_token";

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
  filter: boolean;
  filterable?: boolean;
  searchable: boolean;
  sortable?: boolean;
  hidden?: boolean;
  widget: string;
  placeholder?: string;
  section?: string;
  width?: string;
  order?: number;
  enum_values?: string[];
  relation?: string;
  relation_kind?: string;
  inverse_field?: string;
  readonly: boolean;
};

export type UiEntity = {
  entity: string;
  label: string;
  label_plural: string;
  slug: string;
  searchable: boolean;
  workflow?: string;
  display_field?: string;
  module?: string;
  fields: UiField[];
};

export type TenantConfig = {
  branding: {
    logo?: string | null;
    favicon?: string | null;
    primary_color?: string | null;
    app_name?: string | null;
  };
  ui_config: {
    navigation: string[];
    hidden_entities: string[];
    default_dashboard?: string | null;
  };
  enabled_apps: string[];
  business_config: unknown;
};

export type WorkflowAction = {
  name: string;
  label?: string;
  from: string;
  to: string;
  allowed_roles?: string[];
};

export type EntityAction = {
  name: string;
  label?: string;
  entity?: string;
  style?: string;
  requires_confirmation?: boolean;
};

export type FieldError = { field: string; code?: string; message: string };

export class ApiError extends Error {
  status: number;
  fields: FieldError[];

  constructor(message: string, status: number, fields: FieldError[] = []) {
    super(message);
    this.status = status;
    this.fields = fields;
  }
}

export type Expanded = { id: string; label: string; slug: string; entity: string };

async function request<T>(path: string, init: RequestInit = {}): Promise<T> {
  const token = localStorage.getItem(TOKEN_KEY);
  const headers = new Headers(init.headers);
  headers.set("Content-Type", "application/json");
  if (token) headers.set("Authorization", `Bearer ${token}`);
  const res = await fetch(path, { ...init, headers });
  if (res.status === 204) return undefined as T;
  const data = await res.json().catch(() => ({}));
  if (!res.ok) {
    const fields: FieldError[] =
      data?.details?.fields ?? data?.fields ?? [];
    throw new ApiError(data.message || data.error || res.statusText, res.status, fields);
  }
  return data as T;
}

export const api = {
  login: (email: string, password: string) =>
    request<{ access_token: string; roles: string[] }>("/api/v1/auth/login", {
      method: "POST",
      body: JSON.stringify({ email, password }),
    }),
  register: (body: Record<string, string>) =>
    request<{ access_token: string }>("/api/v1/auth/register", {
      method: "POST",
      body: JSON.stringify(body),
    }),
  me: () =>
    request<{ user: { name: string; email: string }; roles: string[]; tenant_id: string }>(
      "/api/v1/auth/me",
    ),
  ui: () => request<{ entities: UiEntity[] }>("/api/v1/meta/ui"),
  tenantConfig: () => request<TenantConfig>("/api/v1/tenants/me/config"),
  saveTenantConfig: (body: TenantConfig) =>
    request<TenantConfig>("/api/v1/tenants/me/config", {
      method: "PATCH",
      body: JSON.stringify(body),
    }),
  dashboards: () =>
    request<{ dashboards: Array<{ name: string; label: string; module?: string }> }>(
      "/api/v1/meta/dashboards",
    ),
  dashboard: (name: string) =>
    request<{
      name: string;
      label: string;
      cards: Array<{ title: string; entity: string; metric: string; value: number }>;
    }>(`/api/v1/dashboards/${name}`),
  list: (slug: string, params: URLSearchParams) =>
    request<{ items: Record<string, unknown>[]; total: number; page: number; page_size: number }>(
      `/api/v1/${slug}?${params}`,
    ),
  get: (slug: string, id: string) => request<Record<string, unknown>>(`/api/v1/${slug}/${id}`),
  create: (slug: string, body: unknown) =>
    request<Record<string, unknown>>(`/api/v1/${slug}`, { method: "POST", body: JSON.stringify(body) }),
  update: (slug: string, id: string, body: unknown) =>
    request<Record<string, unknown>>(`/api/v1/${slug}/${id}`, {
      method: "PATCH",
      body: JSON.stringify(body),
    }),
  remove: (slug: string, id: string) => request<void>(`/api/v1/${slug}/${id}`, { method: "DELETE" }),
  transition: (slug: string, id: string, transition: string) =>
    request<Record<string, unknown>>(`/api/v1/${slug}/${id}/transition`, {
      method: "POST",
      body: JSON.stringify({ transition }),
    }),
  action: (slug: string, id: string, name: string, body: unknown = {}) =>
    request<Record<string, unknown>>(`/api/v1/${slug}/${id}/actions/${name}`, {
      method: "POST",
      body: JSON.stringify(body ?? {}),
    }),
  workflow: (slug: string, id: string) =>
    request<{ current: string; transitions: WorkflowAction[] }>(`/api/v1/${slug}/${id}/workflow`),
};

export function saveToken(token: string) {
  localStorage.setItem(TOKEN_KEY, token);
}

export function clearToken() {
  localStorage.removeItem(TOKEN_KEY);
}

export function hasToken() {
  return Boolean(localStorage.getItem(TOKEN_KEY));
}

export function listVisible(field: UiField) {
  return (field.list_visible ?? field.list) && !field.hidden;
}

export function formVisible(field: UiField) {
  return (field.form_visible ?? field.form) && !field.hidden && !field.readonly;
}

export function expandedLabel(row: Record<string, unknown>, field: string): string | null {
  const expanded = row._expanded as Record<string, Expanded> | undefined;
  const rel = expanded?.[field];
  return rel?.label ?? null;
}
