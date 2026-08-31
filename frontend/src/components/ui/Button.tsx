import type { ButtonHTMLAttributes, ReactNode } from "react";
import { buttonClass, type ButtonVariant } from "../../theme/tokens";

export function Button({
  variant = "filled",
  className,
  children,
  type = "button",
  ...rest
}: ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: ButtonVariant;
  children?: ReactNode;
}) {
  const classes = [buttonClass(variant), className].filter(Boolean).join(" ");
  return (
    <button type={type} className={classes} {...rest}>
      {children}
    </button>
  );
}
