export function relativeTime(value: unknown, locale = "en"): string {
  if (value == null || value === "") return "";
  const date = new Date(String(value));
  if (Number.isNaN(date.getTime())) return String(value);
  const diff = (date.getTime() - Date.now()) / 1000;
  const abs = Math.abs(diff);
  const rtf = new Intl.RelativeTimeFormat(locale, { numeric: "auto" });
  if (abs < 60) return rtf.format(Math.round(diff), "second");
  if (abs < 3600) return rtf.format(Math.round(diff / 60), "minute");
  if (abs < 86400) return rtf.format(Math.round(diff / 3600), "hour");
  if (abs < 86400 * 30) return rtf.format(Math.round(diff / 86400), "day");
  return date.toLocaleDateString(locale, { day: "numeric", month: "short", year: "numeric" });
}

export function statusTone(status: string): "neutral" | "info" | "success" | "warning" | "danger" {
  const s = status.toLowerCase();
  if (/(complete|done|paid|active|confirmed|approved|resolved|success|sent|delivered)/.test(s)) return "success";
  if (/(pending|draft|open|new|queued)/.test(s)) return "warning";
  if (/(cancel|fail|void|reject|overdue|closed|dead.?letter)/.test(s)) return "danger";
  if (/(progress|assigned|preparing|seated|submitted)/.test(s)) return "info";
  return "neutral";
}

export function fileSize(bytes: unknown): string {
  const n = Number(bytes);
  if (!Number.isFinite(n) || n < 0) return "";
  if (n < 1024) return `${Math.round(n)} B`;
  if (n < 1024 * 1024) {
    const kb = n / 1024;
    return `${kb >= 10 ? Math.round(kb) : kb.toFixed(1)} KB`;
  }
  const mb = n / (1024 * 1024);
  return `${mb >= 10 ? Math.round(mb) : mb.toFixed(1)} MB`;
}

export function fileIcon(filename: string, mime?: string): string {
  const name = filename.toLowerCase();
  const type = (mime ?? "").toLowerCase();
  if (type.startsWith("image/") || /\.(png|jpe?g|gif|webp|svg)$/.test(name)) return "🖼";
  if (type.includes("pdf") || name.endsWith(".pdf")) return "📄";
  if (type.includes("csv") || name.endsWith(".csv")) return "📊";
  return "📎";
}

export function csvEscape(value: unknown): string {
  const s = value == null ? "" : String(value);
  if (/[",\n]/.test(s)) return `"${s.replace(/"/g, '""')}"`;
  return s;
}

export function isoDate(d: Date): string {
  const y = d.getFullYear();
  const m = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${y}-${m}-${day}`;
}

export function datePresetRange(preset: string, now = new Date()): { from: string; to: string } | null {
  const start = new Date(now);
  start.setHours(0, 0, 0, 0);
  const end = new Date(start);
  const addDays = (n: number) => {
    const x = new Date(start);
    x.setDate(x.getDate() + n);
    return x;
  };
  switch (preset) {
    case "today":
      return { from: isoDate(start), to: isoDate(start) };
    case "yesterday": {
      const y = addDays(-1);
      return { from: isoDate(y), to: isoDate(y) };
    }
    case "this_week": {
      const day = (start.getDay() + 6) % 7;
      const from = addDays(-day);
      return { from: isoDate(from), to: isoDate(start) };
    }
    case "this_month": {
      const from = new Date(start.getFullYear(), start.getMonth(), 1);
      return { from: isoDate(from), to: isoDate(start) };
    }
    case "last_7_days":
      return { from: isoDate(addDays(-6)), to: isoDate(start) };
    case "last_30_days":
      return { from: isoDate(addDays(-29)), to: isoDate(start) };
    default:
      return null;
  }
}

/** Derived due label. Does not persist overdue state. */
export function dueChip(
  value: unknown,
  status?: unknown,
  now = new Date(),
): "Overdue" | "Due today" | "Due tomorrow" | null {
  if (value == null || value === "") return null;
  const st = String(status ?? "").toLowerCase();
  if (st === "completed" || st === "cancelled") return null;
  const date = new Date(String(value));
  if (Number.isNaN(date.getTime())) return null;
  const start = new Date(now);
  start.setHours(0, 0, 0, 0);
  const dueDay = new Date(date);
  dueDay.setHours(0, 0, 0, 0);
  const diff = Math.round((dueDay.getTime() - start.getTime()) / 86400000);
  if (date.getTime() < now.getTime() && diff < 0) return "Overdue";
  if (diff < 0) return "Overdue";
  if (diff === 0) return "Due today";
  if (diff === 1) return "Due tomorrow";
  return null;
}

export function downloadCsv(filename: string, headers: string[], rows: unknown[][]) {
  const lines = [
    headers.map(csvEscape).join(","),
    ...rows.map((row) => row.map(csvEscape).join(",")),
  ];
  const blob = new Blob([lines.join("\n")], { type: "text/csv;charset=utf-8" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  a.click();
  URL.revokeObjectURL(url);
}
