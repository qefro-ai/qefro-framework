import type { ButtonHTMLAttributes, ReactNode } from "react";
import { buttonClass } from "../../theme/tokens";

export function IconButton({
  label,
  className,
  children,
  type = "button",
  title,
  ...rest
}: ButtonHTMLAttributes<HTMLButtonElement> & {
  label: string;
  children?: ReactNode;
}) {
  const classes = [buttonClass("icon"), className].filter(Boolean).join(" ");
  return (
    <button type={type} className={classes} aria-label={label} title={title ?? label} {...rest}>
      {children}
    </button>
  );
}
