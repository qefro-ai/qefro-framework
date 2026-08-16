# Accessibility

The generic UI is keyboard-first:

- Skip link to `#main`
- `⌘K` / `Ctrl+K` command palette with arrow/enter
- Labeled inputs (`htmlFor` / `id`), required indicators, `aria-invalid` + error `role="alert"`
- Tabs use `tablist` / `tab` / `aria-selected`
- Dialogs (palette, unsaved changes, quick create) are labeled
- Tables in the list view expose select-all / select-row labels
- Visible `:focus-visible` rings
- Status badges are text, not color alone
- Screen-reader-only loading text on skeletons

Do not rely on placeholder-only fields. Help text is associated with `aria-describedby` when present.
