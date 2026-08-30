/** Material 3–inspired color roles implemented as CSS custom properties in styles.css. */
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

export const MD_ELEVATION_TOKENS = ["--md-level0", "--md-level1", "--md-level2", "--md-level3"] as const;

export const MD_TYPE_TOKENS = [
  "--md-type-display",
  "--md-type-headline",
  "--md-type-title",
  "--md-type-body",
  "--md-type-label",
] as const;

export const MD_MOTION_TOKENS = ["--md-duration-fast", "--md-duration", "--md-easing"] as const;

/** Tenant branding written by App.tsx; M3 primary is derived from these. */
export const BRAND_INPUT_TOKENS = ["--accent", "--primary", "--secondary", "--accent-ink"] as const;

export const BUTTON_VARIANTS = ["filled", "tonal", "outlined", "text", "icon", "destructive"] as const;

export function buttonClass(variant: (typeof BUTTON_VARIANTS)[number]): string {
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
