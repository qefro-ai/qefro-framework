import { NavLink, useLocation, useNavigate } from "react-router-dom";
import { useEffect, useMemo, useState, type ReactNode } from "react";
import { api, clearToken, type UiEntity } from "../../api";
import type { WorkspaceNavItem } from "../../metadata/types";
import { workspaceItemHref } from "../../metadata/navigation";
import { usePrefs } from "../../prefsContext";
import { useRealtime } from "../../realtime";
import NotificationBell from "../NotificationBell";
import { Breadcrumbs } from "./Breadcrumbs";
import { BreadcrumbRecordProvider } from "./breadcrumbContext";
import CommandPalette from "./CommandPalette";

export function AppShell({
  appName,
  logo,
  navEntities,
  workspaceNav,
  allEntities,
  studio,
  userName,
  userEmail,
  roles,
  children,
}: {
  appName: string;
  logo?: string | null;
  navEntities: UiEntity[];
  workspaceNav?: WorkspaceNavItem[];
  allEntities?: UiEntity[];
  studio: boolean;
  userName: string;
  userEmail: string;
  roles: string[];
  children: ReactNode;
}) {
  const { prefs, setTheme, setDensity, setSidebarCollapsed, theme } = usePrefs();
  const navigate = useNavigate();
  const location = useLocation();
  const [palette, setPalette] = useState(false);
  const [mobileOpen, setMobileOpen] = useState(false);
  const [userOpen, setUserOpen] = useState(false);
  const [helpOpen, setHelpOpen] = useState(false);
  const [isMobile, setIsMobile] = useState(() => window.matchMedia("(max-width: 840px)").matches);
  const { connected } = useRealtime({}, () => undefined);

  useEffect(() => {
    const mq = window.matchMedia("(max-width: 840px)");
    const onChange = () => setIsMobile(mq.matches);
    mq.addEventListener("change", onChange);
    return () => mq.removeEventListener("change", onChange);
  }, []);

  useEffect(() => {
    setMobileOpen(false);
    setUserOpen(false);
  }, [location.pathname]);

  const groupedWorkspace = useMemo(() => {
    if (!workspaceNav?.length) return [];
    const hasSections = workspaceNav.some((item) => item.section);
    if (!hasSections) return [["", workspaceNav] as const];
    const map = new Map<string, typeof workspaceNav>();
    for (const item of workspaceNav) {
      const key = item.section || "Workspace";
      const list = map.get(key) ?? [];
      list.push(item);
      map.set(key, list);
    }
    return Array.from(map.entries());
  }, [workspaceNav]);

  const groups = useMemo(() => {
    const map = new Map<string, UiEntity[]>();
    for (const entity of navEntities) {
      const key = entity.module || "Workspace";
      const list = map.get(key) ?? [];
      list.push(entity);
      map.set(key, list);
    }
    return Array.from(map.entries());
  }, [navEntities]);

  const navOpen = isMobile ? mobileOpen : !prefs.sidebarCollapsed;

  return (
    <div className={`shell ${navOpen ? "nav-open" : "nav-collapsed"} ${isMobile ? "is-mobile" : ""}`}>
      <a className="skip-link" href="#main">
        Skip to content
      </a>
      <header className="topbar">
        <button
          type="button"
          className="ghost icon-btn"
          aria-label={navOpen ? "Collapse navigation" : "Open navigation"}
          onClick={() => (isMobile ? setMobileOpen((v) => !v) : setSidebarCollapsed(!prefs.sidebarCollapsed))}
        >
          ☰
        </button>
        <div className="topbar-brand">
          {logo ? (
            <img src={logo} alt="" className="logo" />
          ) : (
            <span className="brand-mark" aria-hidden>
              {(appName.trim().charAt(0) || "Q").toUpperCase()}
            </span>
          )}
          <strong>{appName}</strong>
        </div>
        <button type="button" className="ghost search-trigger" onClick={() => setPalette(true)}>
          Search… <kbd>⌘K</kbd>
        </button>
        <div className="topbar-end">
          <span
            className={`conn-dot ${connected ? "is-on" : ""}`}
            title={connected ? "Live updates connected" : "Reconnecting…"}
            aria-label={connected ? "Realtime connected" : "Realtime disconnected"}
          />
          <NotificationBell entities={navEntities} />
          <button type="button" className="ghost" onClick={() => setHelpOpen(true)}>
            Help
          </button>
          <div className="user-menu">
            <button type="button" className="ghost" aria-expanded={userOpen} onClick={() => setUserOpen((v) => !v)}>
              {userName || "User"}
            </button>
            {userOpen ? (
              <div className="user-panel" role="menu">
                <div className="muted">
                  {userEmail}
                  {roles.length ? <div>{roles.join(", ")}</div> : null}
                </div>
                <label>
                  Theme
                  <select value={prefs.theme} onChange={(e) => setTheme(e.target.value as typeof prefs.theme)}>
                    <option value="system">System</option>
                    <option value="light">Light</option>
                    <option value="dark">Dark</option>
                  </select>
                </label>
                <label>
                  Density
                  <select
                    value={prefs.density}
                    onChange={(e) => setDensity(e.target.value as typeof prefs.density)}
                  >
                    <option value="comfortable">Comfortable</option>
                    <option value="compact">Compact</option>
                  </select>
                </label>
                <button
                  type="button"
                  className="ghost"
                  onClick={async () => {
                    try {
                      await api.logout();
                    } catch {
                      /* still drop the local token */
                    }
                    clearToken();
                    navigate("/login");
                  }}
                >
                  Log out
                </button>
              </div>
            ) : null}
          </div>
        </div>
      </header>
      {isMobile && mobileOpen ? (
        <button type="button" className="nav-backdrop" aria-label="Close navigation" onClick={() => setMobileOpen(false)} />
      ) : null}
      <aside className="nav" aria-label="Application">
        <div className="nav-links">
          <NavLink to="/" className={({ isActive }) => (isActive ? "active" : "")} end>
            <span className="nav-label">Dashboard</span>
          </NavLink>
          {groupedWorkspace.length > 0
            ? groupedWorkspace.map(([section, items]) => (
                <div key={section || "workspace"} className="nav-group">
                  {section ? <div className="nav-group-label">{section}</div> : null}
                  {items.map((item) => {
                    const to = workspaceItemHref(item);
                    return (
                      <NavLink
                        key={`${item.label}-${to}`}
                        to={to}
                        className={({ isActive }) => (isActive ? "active" : "")}
                        title={item.label}
                      >
                        <span className="nav-label">{item.label}</span>
                      </NavLink>
                    );
                  })}
                </div>
              ))
            : groups.map(([group, items]) => (
            <div key={group} className="nav-group">
              <div className="nav-group-label">{group}</div>
              {items.map((e) => (
                <NavLink
                  key={e.slug}
                  to={`/${e.slug}`}
                  className={({ isActive }) => (isActive ? "active" : "")}
                  title={e.label_plural}
                >
                  <span className="nav-label">{e.label_plural}</span>
                </NavLink>
              ))}
            </div>
          ))}
          <NavLink to="/reports" className={({ isActive }) => (isActive ? "active" : "")}>
            <span className="nav-label">Reports</span>
          </NavLink>
          <div className="nav-group">
            <div className="nav-group-label">Administration</div>
            <NavLink to="/settings" className={({ isActive }) => (isActive ? "active" : "")}>
              <span className="nav-label">Settings</span>
            </NavLink>
            {roles.some((r) => r.toLowerCase() === "admin") ? (
              <NavLink to="/settings/audit" className={({ isActive }) => (isActive ? "active" : "")}>
                <span className="nav-label">Audit log</span>
              </NavLink>
            ) : null}
            {studio ? (
              <NavLink to="/studio" className={({ isActive }) => (isActive ? "active" : "")}>
                <span className="nav-label">Studio</span>
              </NavLink>
            ) : null}
          </div>
        </div>
      </aside>
      <main id="main" className="main" tabIndex={-1}>
        <BreadcrumbRecordProvider>
          <Breadcrumbs entities={allEntities ?? navEntities} navSlugs={navEntities.map((e) => e.slug)} />
          {children}
        </BreadcrumbRecordProvider>
      </main>
      <CommandPalette
        entities={allEntities ?? navEntities}
        workspaceNav={workspaceNav}
        studio={studio}
        open={palette}
        onOpenChange={setPalette}
      />
      {helpOpen ? (
        <div className="palette-backdrop" onClick={() => setHelpOpen(false)}>
          <div className="palette dialog" role="dialog" aria-label="Keyboard shortcuts" onClick={(e) => e.stopPropagation()}>
            <h3>Shortcuts</h3>
            <ul className="help-list">
              <li>
                <kbd>⌘</kbd>/<kbd>Ctrl</kbd>+<kbd>K</kbd> Command palette
              </li>
              <li>Search, create records, and jump to lists from metadata.</li>
              <li>Theme and density are saved per signed-in user on this device.</li>
            </ul>
            <div className="dialog-actions">
              <button type="button" className="ghost" onClick={() => setHelpOpen(false)}>
                Close
              </button>
            </div>
          </div>
        </div>
      ) : null}
      <span className="sr-only">{theme} theme</span>
    </div>
  );
}
