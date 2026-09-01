/** Explicit tenant-timezone conversion. API values are UTC. */

export function parseUtc(value: unknown): Date | null {
  if (value == null || value === "") return null;
  const raw = String(value);
  const dt = new Date(raw);
  return Number.isNaN(dt.getTime()) ? null : dt;
}

function partsInZone(date: Date, timeZone: string) {
  const fmt = new Intl.DateTimeFormat("en-CA", {
    timeZone: timeZone || "UTC",
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hourCycle: "h23",
  });
  const map: Record<string, string> = {};
  for (const part of fmt.formatToParts(date)) {
    if (part.type !== "literal") map[part.type] = part.value;
  }
  return {
    date: `${map.year}-${map.month}-${map.day}`,
    time: `${map.hour}:${map.minute}`,
    second: map.second ?? "00",
  };
}

export function utcToLocalParts(value: unknown, timeZone: string): { date: string; time: string } {
  const dt = parseUtc(value);
  if (!dt) return { date: "", time: "" };
  return partsInZone(dt, timeZone || "UTC");
}

export function utcToDatetimeLocal(value: unknown, timeZone: string): string {
  const { date, time } = utcToLocalParts(value, timeZone);
  return date && time ? `${date}T${time}` : "";
}

/**
 * Interpret a naive local datetime in `timeZone` and return UTC ISO-8601.
 * Does not use the browser's local timezone.
 */
export function localToUtcIso(date: string, time: string, timeZone: string): string | null {
  if (!date || !time) return null;
  const hhmm = time.length === 5 ? `${time}:00` : time;
  const asUtc = Date.parse(`${date}T${hhmm}Z`);
  if (Number.isNaN(asUtc)) return null;
  const zone = timeZone || "UTC";
  const utcDate = new Date(asUtc);
  const shown = partsInZone(utcDate, zone);
  const shownUtc = Date.parse(`${shown.date}T${shown.time}:00Z`);
  const offset = asUtc - shownUtc;
  return new Date(asUtc + offset).toISOString();
}

export function formatMoney(value: unknown, currency: string, locale: string, precision = 2): string {
  const n = typeof value === "number" ? value : Number(value);
  if (!Number.isFinite(n)) return "";
  try {
    return new Intl.NumberFormat(locale || "en-US", {
      style: "currency",
      currency: currency || "USD",
      minimumFractionDigits: precision,
      maximumFractionDigits: precision,
    }).format(n);
  } catch {
    return n.toFixed(precision);
  }
}
