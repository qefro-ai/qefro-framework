import { createElement, useEffect } from "react";
import { Navigate, Route, Routes, useLocation, useParams } from "react-router-dom";
import type { TenantConfig, UiEntity } from "../sdk/client";
import type { WorkspaceShortcut } from "../metadata/types";
import { emitUiEvent } from "../core/events";
import { defaultExtensions } from "../core/extensions";
import { EntityListRenderer, EntityFormRenderer, EntityDetailRenderer } from "../renderer";
import Dashboard from "../pages/Dashboard";
import ComposedPage from "../pages/ComposedPage";
import Settings from "../pages/Settings";
import AuditLog from "../pages/AuditLog";
import Reports from "../pages/Reports";
import PublicForm from "../pages/PublicForm";
import Login from "../pages/Login";

export function RouteChangeListener() {
  const location = useLocation();
  useEffect(() => {
    emitUiEvent("route:change", { pathname: location.pathname, search: location.search });
  }, [location.pathname, location.search]);
  return null;
}

function CustomOrComposedPage({ entities }: { entities: UiEntity[] }) {
  const { name } = useParams();
  const custom = defaultExtensions.getPage(name);
  if (custom) return createElement(custom.component, { entities, name });
  return createElement(ComposedPage, { entities });
}

export function QefroRoutes({
  entities,
  config,
  shortcuts,
  navSlugs,
  hiddenEntities,
  roles,
  onConfigSaved,
}: {
  entities: UiEntity[];
  config: TenantConfig | null;
  shortcuts?: WorkspaceShortcut[];
  navSlugs?: string[];
  hiddenEntities?: string[];
  roles?: string[];
  onConfigSaved?: (next: TenantConfig) => void;
}) {
  const extra = [...defaultExtensions.pages.values()].filter((page) => page.path && page.path !== `/pages/${page.name}`);

  return (
    <>
      <RouteChangeListener />
      <Routes>
        <Route path="/" element={<Dashboard entities={entities} config={config} shortcuts={shortcuts} />} />
        <Route path="/login" element={<Navigate to="/" replace />} />
        <Route
          path="/settings"
          element={
            <Settings
              config={config}
              entities={entities}
              navSlugs={navSlugs ?? []}
              hiddenEntities={hiddenEntities ?? []}
              roles={roles ?? []}
              onSaved={onConfigSaved ?? (() => undefined)}
            />
          }
        />
        <Route path="/settings/audit" element={<AuditLog />} />
        <Route path="/reports" element={<Reports />} />
        <Route path="/pages/:name" element={<CustomOrComposedPage entities={entities} />} />
        <Route path="/p/:tenant/:form" element={<PublicForm />} />
        {extra.map((page) => (
          <Route
            key={page.name}
            path={page.path}
            element={createElement(page.component, { entities, name: page.name })}
          />
        ))}
        <Route path="/:slug" element={<EntityListRenderer entities={entities} />} />
        <Route path="/:slug/new" element={<EntityFormRenderer entities={entities} />} />
        <Route path="/:slug/:id" element={<EntityDetailRenderer entities={entities} />} />
        <Route path="/:slug/:id/edit" element={<EntityFormRenderer entities={entities} />} />
        <Route path="*" element={<Navigate to="/" replace />} />
      </Routes>
    </>
  );
}

export function QefroPublicRoutes() {
  return (
    <Routes>
      <Route path="/login" element={<Login />} />
      <Route path="/p/:tenant/:form" element={<PublicForm />} />
      <Route path="*" element={<Navigate to="/login" replace />} />
    </Routes>
  );
}
