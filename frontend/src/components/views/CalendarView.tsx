import { useMemo } from "react";
import { Link, useNavigate, useSearchParams } from "react-router-dom";
import { api } from "../../api";
import { isoDate } from "../../format";
import { friendlyError } from "../../friendlyError";
import { calendarEndField, calendarStartField, calendarTimeField, displayValue } from "../../metadata/views";
import { localToUtcIso, utcToLocalParts } from "../../metadata/timezone";
import { useTenantTheme } from "../../metadata/context";
import type { CollectionViewProps } from "../../views/registry";

type Mode = "day" | "week" | "month";

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
  const titleField = meta.views?.calendar?.title || meta.display_field || "name";
  const subtitleField = meta.views?.calendar?.subtitle || "status";
  const startName = startField?.name || "created_at";
  const locked = (status: string) => Boolean(status && meta.document?.lock_states?.includes(status));
  const hours = Array.from({ length: 14 }, (_, i) => i + 8);

  const range = useMemo(() => {
    if (mode === "day") return { from: new Date(cursor), days: 1 };
    if (mode === "week") return { from: startOfWeek(cursor), days: 7 };
    const from = new Date(cursor.getFullYear(), cursor.getMonth(), 1);
    const startPad = startOfWeek(from);
    return { from: startPad, days: 42 };
  }, [mode, cursor]);

  const days = useMemo(
    () => Array.from({ length: range.days }, (_, i) => addDays(range.from, i)),
    [range],
  );

  const events = rows
    .map((row) => ({
      row,
      when: eventWhen(row, startName, timeField?.name, theme.timezone),
    }))
    .filter((e) => e.when && !Number.isNaN(e.when.getTime()));

  function setMode(next: Mode) {
    const p = new URLSearchParams(params);
    p.set("cal", next);
    setParams(p);
  }

  function shift(dir: number) {
    const p = new URLSearchParams(params);
    const next = new Date(cursor);
    if (mode === "day") next.setDate(next.getDate() + dir);
    else if (mode === "week") next.setDate(next.getDate() + dir * 7);
    else next.setMonth(next.getMonth() + dir);
    p.set("cursor", isoDate(next));
    setParams(p);
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

  async function reschedule(id: string, day: Date) {
    const row = rows.find((r) => String(r.id) === id);
    if (!row || !startField) return;
    if (startField.readonly || locked(String(row.status ?? ""))) {
      onError("Cannot reschedule this record.");
      return;
    }
    try {
      const date = isoDate(day);
      if (startField.type === "datetime" || startField.widget === "datetime") {
        const time = utcToLocalParts(row[startName], theme.timezone).time || "09:00";
        const iso = localToUtcIso(date, time, theme.timezone);
        await api.update(slug, id, { [startName]: iso });
      } else {
        const body: Record<string, unknown> = { [startName]: date };
        if (endField && !endField.readonly) body[endField.name] = date;
        await api.update(slug, id, body);
      }
      onReload();
    } catch (err) {
      onError(friendlyError(err));
      onReload();
    }
  }

  function eventLink(row: Record<string, unknown>, when: Date) {
    return (
      <Link
        key={String(row.id)}
        to={`/${slug}/${row.id}`}
        className="cal-event"
        draggable
        onDragStart={(e) => e.dataTransfer.setData("text/plain", String(row.id))}
      >
        <strong>
          {when.toLocaleTimeString(theme.locale, { hour: "2-digit", minute: "2-digit" })} {displayValue(row, titleField)}
        </strong>
        <div className="muted">{displayValue(row, subtitleField)}</div>
      </Link>
    );
  }

  if (loading) return <p className="muted">Loading calendar…</p>;

  return (
    <div className={`calendar cal-${mode}`}>
      <div className="row">
        <div className="view-selector" role="tablist" aria-label="Calendar range">
          {(["day", "week", "month"] as Mode[]).map((m) => (
            <button key={m} type="button" className={mode === m ? "" : "ghost"} onClick={() => setMode(m)}>
              {m[0].toUpperCase() + m.slice(1)}
            </button>
          ))}
        </div>
        <div className="actions">
          <button type="button" className="ghost" onClick={() => shift(-1)}>
            Prev
          </button>
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
          {days.map((day) => {
            const key = isoDate(day);
            const items = events.filter((e) => isoDate(e.when!) === key);
            return (
              <div
                key={key}
                className="cal-day"
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
      ) : (
        <div className={`cal-agenda cal-${mode}`}>
          {days.map((day) => {
            const key = isoDate(day);
            const items = events.filter((e) => isoDate(e.when!) === key);
            return (
              <section
                key={key}
                className="cal-day"
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
                {hours.map((hour) => {
                  const slot = items.filter((e) => e.when!.getHours() === hour);
                  return (
                    <div key={hour} className="cal-slot">
                      <button type="button" className="ghost cal-hour" onClick={() => openCreate(day, hour)}>
                        {String(hour).padStart(2, "0")}:00
                      </button>
                      <div className="cal-slot-events">{slot.map(({ row, when }) => eventLink(row, when!))}</div>
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
