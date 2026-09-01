/**
 * Chrome strings for the generic renderer. Tenant terminology may override
 * keys; do not hardcode English in new surfaces when a key exists here.
 */
const CHROME: Record<string, string> = {
  cancel: "Cancel",
  "bulk.selected": "{count} selected",
  "bulk.export": "Export selected",
  "bulk.delete": "Delete selected",
  "bulk.archive": "Archive selected",
  "bulk.assign": "Assign…",
  "bulk.clear": "Clear selection",
  "bulk.deleteTitle": "Delete {count}?",
  "bulk.deleteConfirm": "This cannot be undone.",
  "bulk.archiveTitle": "Archive {count}?",
  "bulk.archiveConfirm": "Archived records leave this list. You can restore them later.",
  "bulk.assignTitle": "Assign {count}",
  "bulk.assignHint": "Search for a person, or unassign to clear ownership.",
  "bulk.assignConfirm": "Assign",
  "bulk.unassign": "Unassign",
  "bulk.searchUsers": "Search users",
  "bulk.assignUnavailable": "You don’t have permission to search users.",
  "bulk.done.archive": "Archived {count}",
  "bulk.done.delete": "Deleted {count}",
  "bulk.done.assign": "Assigned {count}",
  "bulk.done.update": "Updated {count}",
  "bulk.done.restore": "Restored {count}",
  "bulk.partial": "{done} · {failed} failed",
  "bulk.failed": "Could not {action} {count}",
  "record.deleteTitle": "Delete {entity}",
  "record.deleteConfirm": "Delete this {entity}? This cannot be undone.",
  "record.archiveTitle": "Archive {entity}",
  "record.archiveConfirm": "Archive this {entity}? It will leave the list until restored.",
  "record.restoreTitle": "Restore {entity}",
  "record.restoreConfirm": "Restore this {entity} to the list?",
  "conflict.title": "Record changed",
  "conflict.message": "This record was changed by another user. Reload before saving.",
  "conflict.reload": "Reload",
  "conflict.stay": "Stay",
  "export.label": "Export",
  "comment.placeholder": "Write a comment… Use @name to mention someone.",
  "list.total": "{count}",
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

/** Lowercase singular/plural noun for a count, e.g. "1 customer" / "3 customers". */
export function entityCount(n: number, label: string, labelPlural: string) {
  const noun = (n === 1 ? label : labelPlural).toLowerCase();
  return `${n} ${noun}`;
}
