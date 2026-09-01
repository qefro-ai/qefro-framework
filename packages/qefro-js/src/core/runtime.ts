import { QefroClient, api, setApiBaseUrl, type TenantConfig, type UiEntity } from "../sdk/client";
import { applyTheme } from "../theme/apply";
import { emitUiEvent, onUiEvent, type UiEventHandler, type UiEventName } from "./events";
import {
  defaultExtensions,
  ExtensionRegistry,
  type ExtensionInput,
  type PageRegistration,
  type ThemeConfig,
} from "./extensions";
import { resolveEntity } from "../metadata/navigation";
import { QefroUI } from "../ui/namespace";
import type { ListOptions } from "../ui/namespace";

export type { ListOptions };

export type QefroOptions = {
  /** API root including version. Default `/api/v1`. */
  apiUrl?: string;
  client?: QefroClient;
  theme?: ThemeConfig;
  /** Do not replace the process-wide default runtime. */
  isolated?: boolean;
};

export type EntityHandle = {
  name: string;
  meta: () => UiEntity | undefined;
  list: (options?: ListOptions) => ReturnType<QefroUI["list"]>;
  form: () => ReturnType<QefroUI["form"]>;
  detail: () => ReturnType<QefroUI["detail"]>;
};

let defaultRuntime: Qefro | null = null;

export function getQefro(): Qefro {
  if (!defaultRuntime) defaultRuntime = new Qefro();
  return defaultRuntime;
}

export class Qefro {
  readonly client: QefroClient;
  readonly extensions: ExtensionRegistry;
  readonly ui: QefroUI;
  private themeConfig: ThemeConfig = {};
  private snapshot: { entities: UiEntity[]; config: TenantConfig | null } = {
    entities: [],
    config: null,
  };

  constructor(options: QefroOptions = {}) {
    const apiUrl = options.apiUrl ?? "/api/v1";
    if (!options.isolated) setApiBaseUrl(apiUrl);
    this.client = options.client ?? api;
    this.extensions = options.isolated ? new ExtensionRegistry() : defaultExtensions;
    if (options.theme) this.theme(options.theme);
    this.ui = new QefroUI(this);
    if (!options.isolated) defaultRuntime = this;
  }

  async init(): Promise<this> {
    applyTheme(this.themeConfig);
    return this;
  }

  hydrate(snapshot: { entities: UiEntity[]; config: TenantConfig | null }) {
    this.snapshot = snapshot;
  }

  getEntities() {
    return this.snapshot.entities;
  }

  getConfig() {
    return this.snapshot.config;
  }

  entity(name: string): EntityHandle {
    const runtime = this;
    return {
      name,
      meta: () => resolveEntity(runtime.getEntities(), name),
      list: (options) => runtime.ui.list(name, options),
      form: () => runtime.ui.form(name),
      detail: () => runtime.ui.detail(name),
    };
  }

  theme(config: ThemeConfig) {
    this.themeConfig = { ...this.themeConfig, ...config };
    applyTheme(this.themeConfig);
    return this;
  }

  getTheme() {
    return this.themeConfig;
  }

  page(name: string, def: PageRegistration) {
    this.extensions.page(name, def);
    return this;
  }

  register(ext: ExtensionInput) {
    this.extensions.apply(ext);
    return this;
  }

  on<K extends UiEventName>(event: K, handler: UiEventHandler<K>) {
    return onUiEvent(event, handler);
  }

  emit<K extends UiEventName>(event: K, payload: Parameters<UiEventHandler<K>>[0]) {
    emitUiEvent(event, payload);
  }
}
