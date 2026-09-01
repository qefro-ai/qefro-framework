import type { ReactNode } from "react";

export function Card({
  children,
  className,
  title,
}: {
  children?: ReactNode;
  className?: string;
  title?: string;
}) {
  return (
    <div className={["card", className].filter(Boolean).join(" ")}>
      {title ? <div className="muted">{title}</div> : null}
      {children}
    </div>
  );
}
