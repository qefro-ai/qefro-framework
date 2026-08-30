/**
 * Chrome strings for the generic renderer. Tenant terminology may override
 * keys; do not hardcode English in new surfaces when a key exists here.
 */
const CHROME: Record<string, string> = {
  "bulk.selected": "{n} selected",
  "bulk.export": "Export selected",
  "bulk.delete": "Delete selected",
  "bulk.archive": "Archive selected",
  "bulk.assign": "Assign…",
  "bulk.deleteConfirm": "Delete {n} records?",
  "bulk.archiveConfirm": "Archive {n} records?",
  "conflict.title": "Record changed",
  "conflict.message": "Record changed by another user. Reload before saving.",
  "conflict.reload": "Reload",
  "conflict.stay": "Stay",
  "export.label": "Export",
};

export function t(
  key: string,
  vars?: Record<string, string | number>,
  terminology?: Record<string, string>,
): string {
  let text = terminology?.[key] || CHROME[key] || key;
  if (vars) {
    for (const [name, value] of Object.entries(vars)) {
      text = text.replaceAll(`{${name}}`, String(value));
    }
  }
  return text;
}
