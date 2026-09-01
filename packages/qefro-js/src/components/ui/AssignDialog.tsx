import { useEffect, useRef, useState } from "react";
import { api } from "../../sdk/client";
import { t } from "../../i18n";

type UserOption = { id: string; label: string; snippet?: string };

export function AssignDialog({
  open,
  title,
  usersSlug = "users",
  onAssign,
  onUnassign,
  onCancel,
}: {
  open: boolean;
  title: string;
  usersSlug?: string;
  onAssign: (userId: string) => void;
  onUnassign: () => void;
  onCancel: () => void;
}) {
  const [q, setQ] = useState("");
  const [users, setUsers] = useState<UserOption[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (!open) {
      setQ("");
      setSelected(null);
      setUsers([]);
      setError("");
      return;
    }
    inputRef.current?.focus();
    function onKey(event: KeyboardEvent) {
      if (event.key === "Escape") onCancel();
    }
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [open, onCancel]);

  useEffect(() => {
    if (!open) return;
    const handle = window.setTimeout(() => {
      const params = new URLSearchParams();
      if (q) params.set("search", q);
      params.set("page", "1");
      params.set("page_size", "20");
      setLoading(true);
      api
        .list(usersSlug, params)
        .then((result) => {
          setError("");
          setUsers(
            result.items.map((row) => ({
              id: String(row.id),
              label: String(row.name ?? row.title ?? row.email ?? row.id),
              snippet: row.email && String(row.email) !== String(row.name) ? String(row.email) : undefined,
            })),
          );
        })
        .catch(() => {
          setUsers([]);
          setError(t("bulk.assignUnavailable"));
        })
        .finally(() => setLoading(false));
    }, 200);
    return () => window.clearTimeout(handle);
  }, [open, q, usersSlug]);

  if (!open) return null;

  return (
    <div className="palette-backdrop" onClick={onCancel} role="presentation">
      <div
        className="dialog dialog-wide"
        role="dialog"
        aria-modal="true"
        aria-labelledby="assign-title"
        onClick={(e) => e.stopPropagation()}
      >
        <h3 id="assign-title">{title}</h3>
        <p>{t("bulk.assignHint")}</p>
        <div className="dialog-body">
          <input
            ref={inputRef}
            value={q}
            onChange={(e) => setQ(e.target.value)}
            placeholder={t("bulk.searchUsers")}
            aria-label={t("bulk.searchUsers")}
          />
          <ul className="picker-list" role="listbox" aria-label={t("bulk.searchUsers")}>
            {loading ? <li className="muted picker-empty">Loading…</li> : null}
            {!loading && error ? <li className="muted picker-empty">{error}</li> : null}
            {!loading && !error && users.length === 0 ? (
              <li className="muted picker-empty">No users found</li>
            ) : null}
            {users.map((user) => (
              <li key={user.id}>
                <button
                  type="button"
                  role="option"
                  aria-selected={selected === user.id}
                  className={selected === user.id ? "is-active" : undefined}
                  onClick={() => setSelected(user.id)}
                >
                  <strong>{user.label}</strong>
                  {user.snippet ? <span className="muted">{user.snippet}</span> : null}
                </button>
              </li>
            ))}
          </ul>
        </div>
        <div className="dialog-actions">
          <button type="button" className="ghost dialog-action-start" onClick={onUnassign}>
            {t("bulk.unassign")}
          </button>
          <button type="button" className="ghost" onClick={onCancel}>
            {t("cancel")}
          </button>
          <button type="button" disabled={!selected} onClick={() => selected && onAssign(selected)}>
            {t("bulk.assignConfirm")}
          </button>
        </div>
      </div>
    </div>
  );
}
