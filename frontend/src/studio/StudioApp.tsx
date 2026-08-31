import { NavLink, Navigate, Route, Routes, useNavigate } from "react-router-dom";
import { useEffect, useState } from "react";
import { api, notifyMetadata } from "../api";
import Overview from "./pages/Overview";
import Apps from "./pages/Apps";
import Entities from "./pages/Entities";
import Workflows from "./pages/Workflows";
import Permissions from "./pages/Permissions";
import ReportsStudio from "./pages/ReportsStudio";
import PagesStudio from "./pages/PagesStudio";
import CommunicationsStudio from "./pages/CommunicationsStudio";
import System from "./pages/System";
import Platform from "./pages/Platform";
import CommandPalette from "./components/CommandPalette";

export default function StudioApp() {
  const [caps, setCaps] = useState<string[]>([]);
  const [denied, setDenied] = useState(false);
  const navigate = useNavigate();

  useEffect(() => {
    api
      .studioCaps()
      .then((d) => {
        setCaps(d.capabilities);
        if (!d.capabilities.includes("studio.view")) setDenied(true);
      })
      .catch(() => setDenied(true));
  }, []);

  if (denied) {
    return (
      <div className="page">
        <h2>Qefro Studio</h2>
        <p className="error">You are not authorized to open Studio.</p>
        <button className="ghost" onClick={() => navigate("/")}>
          Back to app
        </button>
      </div>
    );
  }

  return (
    <div className="studio-shell">
      <aside className="studio-nav">
        <p className="muted">Qefro Studio</p>
        <div className="nav-group">
          <div className="nav-group-label">Workspace</div>
          <NavLink to="/studio" end>
            Overview
          </NavLink>
          <NavLink to="/studio/apps">Apps</NavLink>
          <NavLink to="/studio/entities">Entities</NavLink>
          <NavLink to="/studio/workflows">Workflows</NavLink>
          <NavLink to="/studio/permissions">Permissions</NavLink>
        </div>
        <div className="nav-group">
          <div className="nav-group-label">Platform</div>
          <NavLink to="/studio/notifications">Notifications</NavLink>
          <NavLink to="/studio/webhooks">Webhooks</NavLink>
          <NavLink to="/studio/automations">Automations</NavLink>
          <NavLink to="/studio/public-forms">Public Forms</NavLink>
        </div>
        <div className="nav-group">
          <div className="nav-group-label">Analytics</div>
          <NavLink to="/studio/reports">Reports</NavLink>
          <NavLink to="/studio/dashboards">Dashboards</NavLink>
          <NavLink to="/studio/pages">Pages</NavLink>
          <NavLink to="/studio/print-formats">Print Formats</NavLink>
          <NavLink to="/studio/communications">Templates</NavLink>
        </div>
        <div className="nav-group">
          <div className="nav-group-label">System</div>
          <NavLink to="/studio/system">System</NavLink>
          <NavLink to="/">Exit Studio</NavLink>
        </div>
      </aside>
      <div className="studio-main">
        <CommandPalette caps={caps} />
        <Routes>
          <Route index element={<Overview />} />
          <Route path="apps" element={<Apps />} />
          <Route path="apps/:app" element={<Apps />} />
          <Route path="entities" element={<Entities caps={caps} />} />
          <Route path="entities/:entity" element={<Entities caps={caps} />} />
          <Route path="workflows" element={<Workflows caps={caps} />} />
          <Route path="workflows/:entity" element={<Workflows caps={caps} />} />
          <Route path="permissions" element={<Permissions caps={caps} />} />
          <Route path="permissions/:entity" element={<Permissions caps={caps} />} />
          <Route path="notifications" element={<Platform kind="notifications" />} />
          <Route path="webhooks" element={<Platform kind="webhooks" />} />
          <Route path="automations" element={<Platform kind="automations" />} />
          <Route path="public-forms" element={<Platform kind="public-forms" />} />
          <Route path="reports" element={<ReportsStudio kind="reports" caps={caps} />} />
          <Route path="reports/:name" element={<ReportsStudio kind="reports" caps={caps} />} />
          <Route path="dashboards" element={<ReportsStudio kind="dashboards" caps={caps} />} />
          <Route path="dashboards/:name" element={<ReportsStudio kind="dashboards" caps={caps} />} />
          <Route path="pages" element={<PagesStudio caps={caps} />} />
          <Route path="pages/:name" element={<PagesStudio caps={caps} />} />
          <Route path="print-formats" element={<ReportsStudio kind="print" caps={caps} />} />
          <Route path="print-formats/:name" element={<ReportsStudio kind="print" caps={caps} />} />
          <Route path="communications" element={<CommunicationsStudio caps={caps} />} />
          <Route path="communications/:name" element={<CommunicationsStudio caps={caps} />} />
          <Route path="system" element={<System caps={caps} />} />
          <Route path="*" element={<Navigate to="/studio" replace />} />
        </Routes>
      </div>
    </div>
  );
}

export function can(caps: string[], cap: string) {
  return caps.includes(cap);
}

export async function publishAndReload(body: unknown) {
  const result = await api.studioPublish(body);
  notifyMetadata();
  return result;
}

export function useStudioEntities() {
  const [entities, setEntities] = useState<Array<Record<string, unknown>>>([]);
  useEffect(() => {
    api.studioEntities().then((d) => setEntities(d.entities));
  }, []);
  return entities;
}

export function groupedEntities(entities: Array<Record<string, unknown>>) {
  const groups = new Map<string, Array<Record<string, unknown>>>();
  for (const e of entities) {
    const app = String(e.module || "unassigned");
    const list = groups.get(app) ?? [];
    list.push(e);
    groups.set(app, list);
  }
  return [...groups.entries()];
}
