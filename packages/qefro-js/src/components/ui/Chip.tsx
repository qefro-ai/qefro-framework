import type { ButtonHTMLAttributes, ReactNode } from "react";

export function Chip({
  selected,
  onRemove,
  removeLabel,
  className,
  children,
  ...rest
}: ButtonHTMLAttributes<HTMLButtonElement> & {
  selected?: boolean;
  onRemove?: () => void;
  removeLabel?: string;
  children?: ReactNode;
}) {
  const classes = ["chip", selected ? "is-selected" : "", className].filter(Boolean).join(" ");
  if (onRemove) {
    return (
      <span className={classes}>
        <button type="button" className="chip-action" {...rest}>
          {children}
        </button>
        <button type="button" className="chip-remove" aria-label={removeLabel || "Remove"} onClick={onRemove}>
          ×
        </button>
      </span>
    );
  }
  return (
    <button type="button" className={classes} aria-pressed={selected || undefined} {...rest}>
      {children}
    </button>
  );
}
