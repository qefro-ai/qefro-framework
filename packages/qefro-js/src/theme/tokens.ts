/**
 * Qefro design tokens — Material 3 roles implemented as CSS custom properties.
 *
 * Hierarchy:
 *   Material 3 language
 *     → --qefro-* tokens (this file + styles.css)
 *       → primitives (Button, Chip, Dialog, Menu, Field)
 *         → business surfaces (EntityList, EntityForm, EntityDetail, …)
 *
 * Migration map (keep Qefro behavior; restyle toward M3):
 *   native <button> / .btn     → M3 filled / tonal / outlined / text / icon / destructive
 *   button.ghost               → M3 outlined
 *   IconButton (.icon-btn)     → M3 icon button
 *   ConfirmDialog              → M3 dialog
 *   ActionMenu                 → M3 menu
 *   RelationPicker             → M3 outlined field + menu/list
 *   StatusBadge                → M3 assist/status chip
 *   .tabs / ViewSelector       → M3 tabs / segmented tabs
 *   SnackbarHost               → M3 snackbar
 *   AppShell .nav              → M3 navigation drawer
 *   .entity-card / dashboard   → M3 outlined/filled card
 *   form widgets               → M3 outlined text-field family
 *   FilterBar .chip            → M3 input chip
 *   CommandPalette             → M3 dialog + list
 */

export const QEFRO_COLOR_ROLES = [
  "--qefro-primary",
  "--qefro-on-primary",
  "--qefro-primary-container",
  "--qefro-on-primary-container",
  "--qefro-secondary",
  "--qefro-on-secondary",
  "--qefro-secondary-container",
  "--qefro-on-secondary-container",
  "--qefro-surface",
  "--qefro-surface-dim",
  "--qefro-surface-bright",
  "--qefro-surface-container-lowest",
  "--qefro-surface-container-low",
  "--qefro-surface-container",
  "--qefro-surface-container-high",
  "--qefro-surface-container-highest",
  "--qefro-on-surface",
  "--qefro-on-surface-variant",
  "--qefro-outline",
  "--qefro-outline-variant",
  "--qefro-error",
  "--qefro-on-error",
  "--qefro-error-container",
  "--qefro-on-error-container",
  "--qefro-scrim",
  "--qefro-inverse-surface",
  "--qefro-inverse-on-surface",
] as const;

/** @deprecated alias of QEFRO_COLOR_ROLES; kept for existing tests and CSS. */
export const MD_COLOR_ROLES = [
  "--md-primary",
  "--md-on-primary",
  "--md-primary-container",
  "--md-on-primary-container",
  "--md-secondary",
  "--md-on-secondary",
  "--md-secondary-container",
  "--md-on-secondary-container",
  "--md-surface",
  "--md-surface-dim",
  "--md-surface-bright",
  "--md-surface-container-lowest",
  "--md-surface-container-low",
  "--md-surface-container",
  "--md-surface-container-high",
  "--md-surface-container-highest",
  "--md-on-surface",
  "--md-on-surface-variant",
  "--md-outline",
  "--md-outline-variant",
  "--md-error",
  "--md-on-error",
  "--md-error-container",
  "--md-on-error-container",
  "--md-success",
  "--md-success-container",
  "--md-warning",
  "--md-warning-container",
  "--md-info",
  "--md-info-container",
  "--md-scrim",
  "--md-inverse-surface",
  "--md-inverse-on-surface",
  "--md-inverse-primary",
] as const;

export const MD_SHAPE_TOKENS = ["--md-shape-sm", "--md-shape-md", "--md-shape-lg"] as const;
export const QEFRO_SHAPE_TOKENS = ["--qefro-shape-sm", "--qefro-shape-md", "--qefro-shape-lg"] as const;

export const MD_ELEVATION_TOKENS = ["--md-level0", "--md-level1", "--md-level2", "--md-level3"] as const;
export const QEFRO_ELEVATION_TOKENS = [
  "--qefro-level0",
  "--qefro-level1",
  "--qefro-level2",
  "--qefro-level3",
] as const;

export const MD_TYPE_TOKENS = [
  "--md-type-display",
  "--md-type-headline",
  "--md-type-title",
  "--md-type-body",
  "--md-type-label",
] as const;
export const QEFRO_TYPE_TOKENS = [
  "--qefro-type-display",
  "--qefro-type-headline",
  "--qefro-type-title",
  "--qefro-type-body",
  "--qefro-type-label",
] as const;

export const MD_MOTION_TOKENS = ["--md-duration-fast", "--md-duration", "--md-easing"] as const;
export const QEFRO_MOTION_TOKENS = ["--qefro-duration-fast", "--qefro-duration", "--qefro-easing"] as const;

/** Tenant branding written by App.tsx; M3 primary is derived from these. */
export const BRAND_INPUT_TOKENS = ["--accent", "--primary", "--secondary", "--accent-ink"] as const;

export const BUTTON_VARIANTS = ["filled", "tonal", "outlined", "text", "icon", "destructive"] as const;

export type ButtonVariant = (typeof BUTTON_VARIANTS)[number];

export function buttonClass(variant: ButtonVariant): string {
  switch (variant) {
    case "filled":
      return "btn";
    case "tonal":
      return "btn tonal";
    case "outlined":
      return "btn ghost";
    case "text":
      return "btn text";
    case "icon":
      return "btn ghost icon-btn";
    case "destructive":
      return "btn danger";
  }
}
