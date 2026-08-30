import { useEffect, useState } from "react";

export type SnackbarTone = "success" | "error" | "info";

type Toast = { id: number; message: string; tone: SnackbarTone };

type Listener = (toasts: Toast[]) => void;

let nextId = 1;
let toasts: Toast[] = [];
const listeners = new Set<Listener>();

function emit() {
  for (const listener of listeners) listener(toasts);
}

export function showSnackbar(message: string, tone: SnackbarTone = "success") {
  const toast: Toast = { id: nextId++, message, tone };
  toasts = [...toasts, toast];
  emit();
  window.setTimeout(() => dismissSnackbar(toast.id), 3200);
  return toast.id;
}

export function dismissSnackbar(id: number) {
  toasts = toasts.filter((toast) => toast.id !== id);
  emit();
}

export function subscribeSnackbars(listener: Listener) {
  listeners.add(listener);
  listener(toasts);
  return () => {
    listeners.delete(listener);
  };
}

export function SnackbarHost() {
  const [items, setItems] = useState<Toast[]>([]);

  useEffect(() => subscribeSnackbars(setItems), []);

  if (items.length === 0) return null;

  return (
    <div className="snackbar-region" aria-live="polite" aria-relevant="additions">
      {items.map((toast) => (
        <div key={toast.id} className={`snackbar snackbar-${toast.tone}`} role="status">
          <span>{toast.message}</span>
          <button
            type="button"
            className="text icon-btn"
            aria-label="Dismiss"
            onClick={() => dismissSnackbar(toast.id)}
          >
            ×
          </button>
        </div>
      ))}
    </div>
  );
}
