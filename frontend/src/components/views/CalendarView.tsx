import { useMemo } from "react";
import type { CSSProperties } from "react";
import { Link, useNavigate, useSearchParams } from "react-router-dom";
import { api } from "../../api";
import { isoDate } from "../../format";
import { friendlyError } from "../../friendlyError";
import { calendarEndField, calendarStartField, calendarTimeField, displayValue } from "../../metadata/views";
import { localToUtcIso, utcToLocalParts } from "../../metadata/timezone";
import { useTenantTheme } from "../../metadata/context";
import { FieldValue } from "../fields/FieldValue";
import { Skeleton } from "../ui/EmptyState";
import type { CollectionViewProps } from "../../views/registry";

type Mode = "day" | "week" | "month" | "agenda";

function startOfWeek(d: Date) {
  const x = new Date(d);
  const day = (x.getDay() + 6) % 7;
  x.setDate(x.getDate() - day);
  x.setHours(0, 0, 0, 0);
  return x;
}

function addDays(d: Date, n: number) {
  const x = new Date(d);
  x.setDate(x.getDate() + n);
  return x;
}

function clockMinutes(value: string | undefined): number | null {
  if (!value) return null;
  const [h, m] = value.split(":").map((p) => Number(p));
  if (Number.isNaN(h)) return null;
  return h * 60 + (Number.isNaN(m) ? 0 : m);
}

function minutesToClock(mins: number) {
  const h = Math.floor(mins / 60);
  const m = mins % 60;
  return `${String(h).padStart(2, "0")}:${String(m).padStart(2, "0")}`;
}

function eventWhen(
  row: Record<string, unknown>,
  startName: string,
  timeName: string | undefined,
  timezone: string,
): Date | null {
  const start = row[startName];
  if (start == null || start === "") return null;
  const raw = String(start);
  if (timeName && row[timeName]) {
    return new Date(`${raw}T${String(row[timeName]).slice(0, 8)}`);
  }
  if (raw.includes("T") || raw.endsWith("Z")) {
    const parts = utcToLocalParts(raw, timezone);
    return parts.date ? new Date(`${parts.date}T${parts.time || "00:00"}`) : new Date(raw);
  }
  return new Date(`${raw}T00:00:00`);
}

function isAllDay(
  row: Record<string, unknown>,
  allDayField: string | undefined,
  timeName: string | undefined,
  when: Date,
) {
  if (allDayField && (row[allDayField] === true || row[allDayField] === "true")) return true;
  if (!timeName) return when.getHours() === 0 && when.getMinutes() === 0;
  return false;
}

function expandedLabel(row: Record<string, unknown>, field: string | undefined) {
  if (!field) return "";
  const expanded = row._expanded as Record<string, { label?: string }> | undefined;
  return expanded?.[field]?.label || "";
}

function statusTone(status: string) {
  const s = status.toLowerCase();
  if (s.includes("cancel") || s.includes("no show") || s.includes("fail")) return "danger";
  if (s.includes("confirm") || s.includes("complete") || s.includes("seated")) return "success";
  if (s.includes("pending") || s.includes("draft")) return "warning";
  return "info";
}

function layoutOverlaps<T extends { startMin: number; endMin: number }>(items: T[]) {
  const cols: number[] = [];
  const sorted = [...items].sort((a, b) => a.startMin - b.startMin || a.endMin - b.endMin);
  const active: Array<{ end: number; col: number }> = [];
  let max = 1;
  const colOf = new Map<T, { col: number; cols: number }>();
  for (const item of sorted) {
    for (let i = active.length - 1; i >= 0; i--) {
      if (active[i].end <= item.startMin) active.splice(i, 1);
    }
    const used = new Set(active.map((a) => a.col));
    let col = 0;
    while (used.has(col)) col += 1;
    active.push({ end: item.endMin, col });
    cols[col] = 1;
    max = Math.max(max, active.length, col + 1);
    colOf.set(item, { col, cols: max });
  }
  for (const item of sorted) {
    const cur = colOf.get(item);
    if (cur) cur.cols = max;
  }
  return colOf;
}

export default function CalendarView({
  meta,
  slug,
  rows,
  loading,
  onReload,
  onError,
}: CollectionViewProps) {
  const theme = useTenantTheme();
  const navigate = useNavigate();
  const [params, setParams] = useSearchParams();
  const mode = ((params.get("cal") as Mode) || "month") as Mode;
  const cursor = params.get("cursor") ? new Date(params.get("cursor") as string) : new Date();
  const startField = calendarStartField(meta);
  const timeField = calendarTimeField(meta);
  const endField = calendarEndField(meta);
  const endTimeName = meta.scheduling?.end_time || meta.views?.calendar?.end;
  const titleField = meta.views?.calendar?.title || meta.display_field || "name";
  const subtitleField = meta.views?.calendar?.subtitle || "status";
  const startName = startField?.name || "created_at";
  const resourceField = meta.scheduling?.resources?.[0];
  const allDayField = meta.scheduling?.all_day;
  const locked = (status: string) => Boolean(status && meta.document?.lock_states?.includes(status));
  const hourStart = meta.scheduling?.day_start_hour ?? 8;
  const hourEnd = Math.max(hourStart + 1, meta.scheduling?.day_end_hour ?? 21);
  const hours = Array.from({ length: hourEnd - hourStart }, (_, i) => i + hourStart);

  const range = useMemo(() => {
    if (mode === "day") return { from: new Date(cursor), days: 1 };
    if (mode === "week" || mode === "agenda") return { from: startOfWeek(cursor), days: 7 };
    const from = new Date(cursor.getFullYear(), cursor.getMonth(), 1);
    const startPad = startOfWeek(from);
    return { from: startPad, days: 42 };
  }, [mode, cursor]);

  const days = useMemo(
    () => Array.from({ length: range.days }, (_, i) => addDays(range.from, i)),
    [range],
  );

  const events = rows
    .map((row) => {
      const when = eventWhen(row, startName, timeField?.name, theme.timezone);
      return {
        row,
        when,
        allDay: when ? isAllDay(row, allDayField, timeField?.name, when) : false,
      };
    })
    .filter((e) => e.when && !Number.isNaN(e.when.getTime()));

  function setMode(next: Mode) {
    const p = new URLSearchParams(params);
    p.set("cal", next);
    setParams(p);
  }

  function setCursor(next: Date) {
    const p = new URLSearchParams(params);
    p.set("cursor", isoDate(next));
    setParams(p);
  }

  function shift(dir: number) {
    const next = new Date(cursor);
    if (mode === "day") next.setDate(next.getDate() + dir);
    else if (mode === "week" || mode === "agenda") next.setDate(next.getDate() + dir * 7);
    else next.setMonth(next.getMonth() + dir);
    setCursor(next);
  }

  function openCreate(day: Date, hour?: number) {
    const q = new URLSearchParams();
    const date = isoDate(day);
    if (startField?.type === "datetime" || startField?.widget === "datetime") {
      const time = `${String(hour ?? 9).padStart(2, "0")}:00`;
      q.set(startName, `${date}T${time}`);
    } else {
      q.set(startName, date);
      if (timeField) q.set(timeField.name, `${String(hour ?? 9).padStart(2, "0")}:00`);
    }
    navigate(`/${slug}/new?${q.toString()}`);
  }

  async function reschedule(id: string, day: Date, hour?: number) {
    const row = rows.find((r) => String(r.id) === id);
    if (!row || !startField) return;
    if (startField.readonly || locked(String(row.status ?? ""))) {
      onError("Cannot reschedule this record.");
      return;
    }
    try {
      const date = isoDate(day);
      const body: Record<string, unknown> = {};
      if (row.updated_at) body._expected_updated_at = row.updated_at;
      if (startField.type === "datetime" || startField.widget === "datetime") {
        const current = utcToLocalParts(row[startName], theme.timezone);
        const time = hour != null ? `${String(hour).padStart(2, "0")}:00` : current.time || "09:00";
        body[startName] = localToUtcIso(date, time, theme.timezone);
        if (endField && !endField.readonly && (endField.type === "datetime" || endField.widget === "datetime")) {
          const endParts = utcToLocalParts(row[endField.name], theme.timezone);
          const startMins = clockMinutes(current.time || "09:00") ?? 9 * 60;
          const endMins = clockMinutes(endParts.time) ?? startMins + (meta.scheduling?.duration_minutes ?? 60);
          const duration = Math.max(endMins - startMins, 15);
          const nextStart = clockMinutes(time) ?? 9 * 60;
          body[endField.name] = localToUtcIso(date, minutesToClock(nextStart + duration), theme.timezone);
        }
      } else {
        body[startName] = date;
        if (timeField && hour != null && !timeField.readonly) {
          const prev = clockMinutes(String(row[timeField.name] ?? "09:00")) ?? 9 * 60;
          const nextStart = hour * 60;
          body[timeField.name] = minutesToClock(nextStart);
          const endName = endTimeName && meta.fields.some((f) => f.name === endTimeName) ? endTimeName : undefined;
          if (endName) {
            const prevEnd = clockMinutes(String(row[endName] ?? "")) ?? prev + (meta.scheduling?.duration_minutes ?? 60);
            body[endName] = minutesToClock(nextStart + Math.max(prevEnd - prev, 15));
          }
        }
        if (endField && !endField.readonly && endField.type === "date") body[endField.name] = date;
      }
      await api.update(slug, id, body);
      onReload();
    } catch (err) {
      onError(friendlyError(err));
      onReload();
    }
  }

  function eventLink(row: Record<string, unknown>, when: Date, extraClass = "") {
    const status = String(row.status ?? "");
    const resource = expandedLabel(row, resourceField);
    const timeLabel = when.toLocaleTimeString(theme.locale, { hour: "2-digit", minute: "2-digit" });
    return (
      <Link
        key={String(row.id)}
        to={`/${slug}/${row.id}`}
        className={`cal-event cal-event-${statusTone(status)}${extraClass ? ` ${extraClass}` : ""}`}
        draggable
        onDragStart={(e) => e.dataTransfer.setData("text/plain", String(row.id))}
      >
        <strong>
          {timeLabel} {displayValue(row, titleField)}
        </strong>
        <div className="muted">
          <FieldValue
            row={row}
            field={meta.fields.find((f) => f.name === subtitleField)}
            fieldName={subtitleField}
          />
          {resource ? ` · ${resource}` : ""}
        </div>
      </Link>
    );
  }

  if (loading && rows.length === 0) return <Skeleton variant="calendar" />;

  const todayKey = isoDate(new Date());
  const cursorMonth = cursor.getMonth();
  const modes: Mode[] = ["day", "week", "month", "agenda"];

  return (
    <div className={`calendar cal-${mode}${loading ? " is-loading" : ""}`} aria-busy={loading || undefined}>
      <div className="row cal-toolbar">
        <div className="view-selector" role="tablist" aria-label="Calendar range">
          {modes.map((m) => (
            <button
              key={m}
              type="button"
              className={mode === m ? "is-active" : "ghost"}
              aria-selected={mode === m}
              onClick={() => setMode(m)}
            >
              {m[0].toUpperCase() + m.slice(1)}
            </button>
          ))}
        </div>
        <div className="actions cal-nav">
          <button type="button" className="ghost" onClick={() => shift(-1)}>
            Prev
          </button>
          <button type="button" className="ghost" onClick={() => setCursor(new Date())}>
            Today
          </button>
          <label className="cal-date">
            <span className="sr-only">Date</span>
            <input
              type="date"
              value={isoDate(cursor)}
              onChange={(e) => {
                if (e.target.value) setCursor(new Date(e.target.value));
              }}
            />
          </label>
          <strong>
            {cursor.toLocaleDateString(theme.locale, {
              month: "long",
              year: "numeric",
              day: mode === "month" ? undefined : "numeric",
            })}
          </strong>
          <button type="button" className="ghost" onClick={() => shift(1)}>
            Next
          </button>
        </div>
      </div>
      {mode === "month" ? (
        <div className="cal-grid cal-month">
          {Array.from({ length: 7 }, (_, i) => addDays(startOfWeek(new Date()), i)).map((day) => (
            <div key={day.getDay()} className="cal-dow">
              {day.toLocaleDateString(theme.locale, { weekday: "short" })}
            </div>
          ))}
          {days.map((day) => {
            const key = isoDate(day);
            const items = events.filter((e) => isoDate(e.when!) === key);
            return (
              <div
                key={key}
                className={`cal-day${key === todayKey ? " is-today" : ""}${day.getMonth() !== cursorMonth ? " is-other-month" : ""}`}
                onDragOver={(e) => e.preventDefault()}
                onDrop={(e) => {
                  e.preventDefault();
                  const id = e.dataTransfer.getData("text/plain");
                  if (id) void reschedule(id, day);
                }}
                onDoubleClick={() => openCreate(day)}
              >
                <header>
                  <button type="button" className="ghost" onClick={() => openCreate(day)}>
                    {day.getDate()}
                  </button>
                </header>
                {items.map(({ row, when }) => eventLink(row, when!))}
              </div>
            );
          })}
        </div>
      ) : mode === "agenda" ? (
        <div className="cal-agenda-list">
          {days.map((day) => {
            const key = isoDate(day);
            const items = events.filter((e) => isoDate(e.when!) === key);
            return (
              <section key={key} className={`cal-agenda-day${key === todayKey ? " is-today" : ""}`}>
                <header>
                  {day.toLocaleDateString(theme.locale, { weekday: "long", month: "short", day: "numeric" })}
                  <button type="button" className="ghost" onClick={() => openCreate(day)}>
                    Add
                  </button>
                </header>
                {items.length === 0 ? <p className="muted">No events</p> : items.map(({ row, when }) => eventLink(row, when!))}
              </section>
            );
          })}
        </div>
      ) : (
        <div className={`cal-agenda cal-${mode}`}>
          {days.map((day) => {
            const key = isoDate(day);
            const items = events.filter((e) => isoDate(e.when!) === key);
            const allDay = items.filter((e) => e.allDay);
            const timed = items.filter((e) => !e.allDay);
            return (
              <section
                key={key}
                className={`cal-day${key === todayKey ? " is-today" : ""}`}
                onDragOver={(e) => e.preventDefault()}
                onDrop={(e) => {
                  e.preventDefault();
                  const id = e.dataTransfer.getData("text/plain");
                  if (id) void reschedule(id, day);
                }}
              >
                <header>
                  {day.toLocaleDateString(theme.locale, { weekday: "short", month: "short", day: "numeric" })}
                </header>
                {allDay.length > 0 ? (
                  <div className="cal-allday">
                    {allDay.map(({ row, when }) => eventLink(row, when!, "is-allday"))}
                  </div>
                ) : null}
                {hours.map((hour) => {
                  const slot = timed
                    .filter((e) => e.when!.getHours() === hour)
                    .map((e) => {
                      const startMin = e.when!.getHours() * 60 + e.when!.getMinutes();
                      const endRaw = endTimeName ? String(e.row[endTimeName] ?? "") : "";
                      const endMin = clockMinutes(endRaw) ?? startMin + (meta.scheduling?.duration_minutes ?? 60);
                      return { ...e, startMin, endMin };
                    });
                  const cols = layoutOverlaps(slot);
                  return (
                    <div
                      key={hour}
                      className="cal-slot"
                      onDragOver={(e) => e.preventDefault()}
                      onDrop={(e) => {
                        e.preventDefault();
                        const id = e.dataTransfer.getData("text/plain");
                        if (id) void reschedule(id, day, hour);
                      }}
                    >
                      <button type="button" className="ghost cal-hour" onClick={() => openCreate(day, hour)}>
                        {String(hour).padStart(2, "0")}:00
                      </button>
                      <div className="cal-slot-events" style={{ "--cal-cols": String(Math.max(1, ...[...cols.values()].map((c) => c.cols))) } as CSSProperties}>
                        {slot.map((item) => {
                          const pos = cols.get(item);
                          return (
                            <div
                              key={String(item.row.id)}
                              className="cal-overlap"
                              style={
                                pos
                                  ? { gridColumn: `${pos.col + 1} / span 1` }
                                  : undefined
                              }
                            >
                              {eventLink(item.row, item.when!)}
                            </div>
                          );
                        })}
                      </div>
                    </div>
                  );
                })}
              </section>
            );
          })}
        </div>
      )}
    </div>
  );
}
