import type { UiEntity, UiField, WidgetOptions, UiWhen } from "./metadata/types";

export type { UiEntity, UiField, WidgetOptions, UiWhen };

const TOKEN_KEY = "qefro_token";

export type TenantConfig = {
  branding: {
    logo?: string | null;
    favicon?: string | null;
    primary_color?: string | null;
    secondary_color?: string | null;
    accent_color?: string | null;
    company_name?: string | null;
    app_name?: string | null;
  };
  ui_config: {
    navigation: string[];
    hidden_entities: string[];
    default_dashboard?: string | null;
    terminology?: Record<string, string>;
  };
  enabled_apps: string[];
  business?: {
    currency?: string;
    timezone?: string;
    locale?: string;
    date_format?: string;
    number_format?: string;
  };
  business_config?: unknown;
  features?: { flags?: Record<string, boolean> };
  plan?: string | null;
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

export function tokenHeader(): Record<string, string> {
  const token = localStorage.getItem(TOKEN_KEY);
  return token ? { Authorization: `Bearer ${token}` } : {};
}

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
  ui: () =>
    request<{
      entities: UiEntity[];
      branding?: TenantConfig["branding"];
      enabled_apps?: string[];
      features?: Record<string, boolean>;
      locale?: string;
      timezone?: string;
      currency?: string;
      navigation?: string[];
      terminology?: Record<string, string>;
      default_dashboard?: string | null;
    }>("/api/v1/meta/ui"),
  tenant: () => request<Record<string, unknown>>("/api/v1/tenant"),
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
      cards: Array<{
        title: string;
        entity: string;
        metric: string;
        kind?: string;
        chart?: string;
        value: number;
        series?: Array<{ label: string; value: number }>;
        items?: Record<string, unknown>[];
        total?: number;
      }>;
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
  audit: (entity: string, entityId: string) =>
    request<{ items: Array<Record<string, unknown>> }>(
      `/api/v1/audit?entity=${encodeURIComponent(entity)}&entity_id=${encodeURIComponent(entityId)}`,
    ),
  upload: (file: File, kind: "file" | "image" = "file", onProgress?: (n: number) => void) =>
    new Promise<{ key: string; url: string; filename: string; content_type: string; size: number }>(
      (resolve, reject) => {
        const xhr = new XMLHttpRequest();
        xhr.open("POST", `/api/v1/files?kind=${kind}`);
        const headers = tokenHeader();
        for (const [k, v] of Object.entries(headers)) xhr.setRequestHeader(k, v);
        xhr.upload.onprogress = (e) => {
          if (e.lengthComputable && onProgress) onProgress(e.loaded / e.total);
        };
        xhr.onload = () => {
          try {
            const data = JSON.parse(xhr.responseText || "{}");
            if (xhr.status >= 400) {
              reject(new ApiError(data.message || xhr.statusText, xhr.status));
            } else {
              resolve(data);
            }
          } catch (err) {
            reject(err);
          }
        };
        xhr.onerror = () => reject(new ApiError("upload failed", 0));
        const body = new FormData();
        body.append("file", file);
        xhr.send(body);
      },
    ),
  savedFilters: (entity: string) =>
    request<{ items: Array<{ id: string; name: string; query: Record<string, unknown> }> }>(
      `/api/v1/saved-filters?entity=${encodeURIComponent(entity)}`,
    ),
  saveFilter: (entity: string, name: string, query: unknown) =>
    request(`/api/v1/saved-filters`, {
      method: "POST",
      body: JSON.stringify({ entity, name, query }),
    }),
  deleteSavedFilter: (id: string) =>
    request<void>(`/api/v1/saved-filters/${id}`, { method: "DELETE" }),
};

const AUTH_EVENT = "qefro-auth";

function notifyAuth() {
  window.dispatchEvent(new Event(AUTH_EVENT));
}

export function saveToken(token: string) {
  localStorage.setItem(TOKEN_KEY, token);
  notifyAuth();
}

export function clearToken() {
  localStorage.removeItem(TOKEN_KEY);
  notifyAuth();
}

export function hasToken() {
  return Boolean(localStorage.getItem(TOKEN_KEY));
}

export function onAuthChange(handler: () => void) {
  window.addEventListener(AUTH_EVENT, handler);
  window.addEventListener("storage", handler);
  return () => {
    window.removeEventListener(AUTH_EVENT, handler);
    window.removeEventListener("storage", handler);
  };
}

export function listVisible(field: UiField) {
  return (field.list_visible ?? field.list) && !field.hidden;
}

export function formVisible(field: UiField) {
  return (field.form_visible ?? field.form) && !field.hidden;
}

export function detailVisible(field: UiField) {
  return (field.detail_visible ?? field.detail ?? true) && !field.hidden;
}

export function expandedLabel(row: Record<string, unknown>, field: string): string | null {
  const expanded = row._expanded as Record<string, Expanded> | undefined;
  const rel = expanded?.[field];
  return rel?.label ?? null;
}
