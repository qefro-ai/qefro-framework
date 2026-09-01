import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { api, type UiEntity } from "../api";
import { relativeTime } from "../format";
import { useRealtime } from "../realtime";
import { useTenantTheme } from "../metadata/context";

type Note = {
  id?: string;
  title?: string;
  body?: string;
  read_at?: string | null;
  entity?: string;
  record_id?: string;
  created_at?: string;
};

export default function NotificationBell({ entities = [] }: { entities?: UiEntity[] }) {
  const [open, setOpen] = useState(false);
  const [unread, setUnread] = useState(0);
  const [items, setItems] = useState<Note[]>([]);
  const navigate = useNavigate();
  const theme = useTenantTheme();

  async function load() {
    const data = await api.notifications();
    setUnread(data.unread ?? 0);
    setItems((data.items as Note[]) ?? []);
  }

  useEffect(() => {
    load().catch(() => undefined);
  }, []);

  useRealtime({}, () => {
    load().catch(() => undefined);
  });

  function openRecord(note: Note) {
    if (!note.entity || !note.record_id) return;
    const slug = entities.find((e) => e.entity === note.entity)?.slug;
    if (slug) navigate(`/${slug}/${note.record_id}`);
  }

  return (
    <div className="notify-wrap">
      <button type="button" className="ghost" onClick={() => setOpen((v) => !v)} aria-label="Notifications">
        🔔 {unread > 0 ? <span className="notify-count">{unread}</span> : null}
      </button>
      {open ? (
        <div className="notify-panel" role="dialog" aria-label="Notifications">
          <h3 className="notify-heading">Notifications</h3>
          {items.length === 0 ? <p className="muted">No notifications.</p> : null}
          <ul>
            {items.map((n) => (
              <li key={String(n.id)} className={n.read_at ? "is-read" : ""}>
                <button
                  type="button"
                  className="ghost"
                  onClick={async () => {
                    if (n.id) await api.readNotification(String(n.id)).catch(() => undefined);
                    setOpen(false);
                    openRecord(n);
                    await load().catch(() => undefined);
                  }}
                >
                  <strong>{n.title || "Notification"}</strong>
                  {n.body ? <div className="muted">{n.body}</div> : null}
                  {n.created_at ? <div className="muted">{relativeTime(n.created_at, theme.locale)}</div> : null}
                </button>
              </li>
            ))}
          </ul>
        </div>
      ) : null}
    </div>
  );
}
