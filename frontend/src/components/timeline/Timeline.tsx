import { utcToDatetimeLocal } from "../../metadata/timezone";
import { relativeTime } from "../../format";

export type TimelineItem = {
  id?: string;
  action?: string;
  event?: string;
  activity_type?: string;
  actor?: string;
  actor_name?: string;
  created_at?: unknown;
  summary?: string;
  message?: string;
};

function kind(item: TimelineItem): string {
  const raw = String(item.activity_type ?? item.action ?? item.event ?? "updated").toLowerCase();
  if (raw.includes("creat")) return "created";
  if (raw.includes("transition") || raw.includes("workflow") || raw.includes("confirm") || raw.includes("cancel")) {
    return "workflow";
  }
  if (raw.includes("attach")) return "attachment";
  if (raw.includes("notif")) return "notification";
  if (raw.includes("comment")) return "comment";
  if (raw.includes("assign")) return "assignment";
  if (raw.includes("integrat") || raw.includes("webhook") || raw === "system") return "system";
  if (raw.includes("updat")) return "updated";
  return raw;
}

function dayLabel(value: unknown, locale?: string): string {
  if (!value) return "";
  const date = new Date(String(value));
  if (Number.isNaN(date.getTime())) return "";
  const today = new Date();
  const start = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
  const diff = (start(today) - start(date)) / 86400000;
  if (diff === 0) return "Today";
  if (diff === 1) return "Yesterday";
  return date.toLocaleDateString(locale || "en-US", { month: "short", day: "numeric", year: "numeric" });
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
  const groups: Array<[string, TimelineItem[]]> = [];
  for (const item of items) {
    const label = dayLabel(item.created_at, locale) || "Earlier";
    const last = groups[groups.length - 1];
    if (last && last[0] === label) last[1].push(item);
    else groups.push([label, [item]]);
  }
  return (
    <ol className="timeline" aria-label="Activity">
      {groups.map(([day, dayItems]) => (
        <li key={day} className="timeline-day">
          <h4 className="timeline-day-label">{day}</h4>
          <ol>
            {dayItems.map((item, i) => {
              const label = String(item.summary ?? item.message ?? item.action ?? item.event ?? "updated");
              const actor = item.actor_name || item.actor;
              const when = item.created_at;
              const time = utcToDatetimeLocal(when, timezone).split("T")[1]?.slice(0, 5);
              return (
                <li key={String(item.id ?? `${day}-${i}`)} className={`timeline-item timeline-${kind(item)}`}>
                  <div className="timeline-time" title={utcToDatetimeLocal(when, timezone).replace("T", " ")}>
                    <span className="timeline-dot" aria-hidden="true" />
                    {time || relativeTime(when, locale)}
                  </div>
                  <div>
                    <div className="timeline-title">{label}</div>
                    {actor ? <div className="muted">{actor}</div> : null}
                    {time ? <div className="muted">{relativeTime(when, locale)}</div> : null}
                  </div>
                </li>
              );
            })}
          </ol>
        </li>
      ))}
    </ol>
  );
}
