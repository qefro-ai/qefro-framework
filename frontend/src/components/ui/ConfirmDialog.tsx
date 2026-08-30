import { useEffect, useRef, type ReactNode } from "react";

export function ConfirmDialog({
  open,
  title,
  message,
  confirmLabel = "Confirm",
  cancelLabel = "Cancel",
  danger,
  confirmDisabled,
  className,
  children,
  onConfirm,
  onCancel,
}: {
  open: boolean;
  title?: string;
  message?: string;
  confirmLabel?: string;
  cancelLabel?: string;
  danger?: boolean;
  confirmDisabled?: boolean;
  className?: string;
  children?: ReactNode;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  const ref = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    if (!open) return;
    if (!children) ref.current?.focus();
    function onKey(event: KeyboardEvent) {
      if (event.key === "Escape") onCancel();
    }
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [open, onCancel, children]);
  if (!open) return null;
  return (
    <div className="palette-backdrop" onClick={onCancel} role="presentation">
      <div
        className={`dialog${className ? ` ${className}` : ""}`}
        role="dialog"
        aria-modal="true"
        aria-labelledby="confirm-title"
        aria-describedby={message ? "confirm-desc" : undefined}
        onClick={(e) => e.stopPropagation()}
      >
        <h3 id="confirm-title">{title || "Confirm"}</h3>
        {message ? <p id="confirm-desc">{message}</p> : null}
        {children ? <div className="dialog-body">{children}</div> : null}
        <div className="dialog-actions">
          <button type="button" className="ghost" onClick={onCancel}>
            {cancelLabel}
          </button>
          <button
            ref={ref}
            type="button"
            className={danger ? "danger" : undefined}
            disabled={confirmDisabled}
            onClick={onConfirm}
          >
            {confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );
}
