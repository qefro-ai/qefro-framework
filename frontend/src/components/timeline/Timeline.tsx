import { utcToDatetimeLocal } from "../../metadata/timezone";
import { relativeTime } from "../../format";

export type TimelineItem = {
  id?: string;
  action?: string;
  event?: string;
  actor?: string;
  created_at?: unknown;
  summary?: string;
};

function kind(item: TimelineItem): string {
  const raw = String(item.action ?? item.event ?? "updated").toLowerCase();
  if (raw.includes("creat")) return "created";
  if (raw.includes("transition") || raw.includes("workflow") || raw.includes("confirm") || raw.includes("cancel")) {
    return "workflow";
  }
  if (raw.includes("attach")) return "attachment";
  if (raw.includes("notif")) return "notification";
  if (raw.includes("comment")) return "comment";
  if (raw.includes("integrat") || raw.includes("webhook")) return "integration";
  if (raw.includes("updat")) return "updated";
  return raw;
}

export function Timeline({
  items,
  timezone,
  locale,
}: {
  items: TimelineItem[];
  timezone: string;
  locale?: string;
}) {
  if (items.length === 0) {
    return <p className="empty">No activity yet.</p>;
  }
  return (
    <ol className="timeline" aria-label="Activity">
      {items.map((item, i) => {
        const label = String(item.summary ?? item.action ?? item.event ?? "updated");
        const when = item.created_at;
        return (
          <li key={String(item.id ?? i)} className={`timeline-item timeline-${kind(item)}`}>
            <div className="timeline-time" title={utcToDatetimeLocal(when, timezone).replace("T", " ")}>
              {relativeTime(when, locale)}
            </div>
            <div>
              <div className="timeline-title">{label}</div>
              {item.actor ? <div className="muted">by {item.actor}</div> : null}
            </div>
          </li>
        );
      })}
    </ol>
  );
}
