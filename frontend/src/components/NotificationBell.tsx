import { useEffect, useState } from "react";
import { api } from "../api";
import { useRealtime } from "../realtime";

type Note = {
  id?: string;
  title?: string;
  body?: string;
  read_at?: string | null;
};

export default function NotificationBell() {
  const [open, setOpen] = useState(false);
  const [unread, setUnread] = useState(0);
  const [items, setItems] = useState<Note[]>([]);

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

  return (
    <div className="notify-wrap">
      <button type="button" className="ghost" onClick={() => setOpen((v) => !v)} aria-label="Notifications">
        🔔 {unread > 0 ? unread : ""}
      </button>
      {open ? (
        <div className="notify-panel" role="dialog" aria-label="Notifications">
          {items.length === 0 ? <p className="muted">No notifications.</p> : null}
          <ul>
            {items.map((n) => (
              <li key={String(n.id)}>
                <button
                  type="button"
                  className="ghost"
                  onClick={async () => {
                    if (n.id) await api.readNotification(String(n.id)).catch(() => undefined);
                    setOpen(false);
                    await load().catch(() => undefined);
                  }}
                >
                  <strong>{n.title || "Notification"}</strong>
                  {n.body ? <div className="muted">{n.body}</div> : null}
                </button>
              </li>
            ))}
          </ul>
        </div>
      ) : null}
    </div>
  );
}
