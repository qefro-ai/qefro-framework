import { useEffect, useId, useRef, useState, type ReactNode } from "react";

export type ActionMenuItem = {
  key: string;
  label: ReactNode;
  onSelect?: () => void;
  href?: string;
  target?: string;
  danger?: boolean;
  hidden?: boolean;
};

export function ActionMenu({
  label = "More",
  items,
}: {
  label?: string;
  items: ActionMenuItem[];
}) {
  const visible = items.filter((item) => !item.hidden);
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const menuId = useId();

  useEffect(() => {
    if (!open) return;
    function onPointer(event: MouseEvent) {
      if (!root.current?.contains(event.target as Node)) setOpen(false);
    }
    function onKey(event: KeyboardEvent) {
      if (event.key === "Escape") setOpen(false);
    }
    document.addEventListener("mousedown", onPointer);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onPointer);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  if (visible.length === 0) return null;

  return (
    <div className="more-menu action-menu" ref={root}>
      <button
        type="button"
        className="ghost"
        aria-expanded={open}
        aria-haspopup="menu"
        aria-controls={menuId}
        onClick={() => setOpen((value) => !value)}
      >
        {label}
      </button>
      {open ? (
        <ul id={menuId} className="menu-list" role="menu">
          {visible.map((item) => (
            <li key={item.key} role="none">
              {item.href ? (
                <a
                  href={item.href}
                  role="menuitem"
                  target={item.target}
                  rel={item.target === "_blank" ? "noreferrer" : undefined}
                  onClick={() => setOpen(false)}
                >
                  {item.label}
                </a>
              ) : (
                <button
                  type="button"
                  role="menuitem"
                  className={item.danger ? "danger" : "ghost"}
                  onClick={() => {
                    setOpen(false);
                    item.onSelect?.();
                  }}
                >
                  {item.label}
                </button>
              )}
            </li>
          ))}
        </ul>
      ) : null}
    </div>
  );
}
