import { statusTone } from "../../format";

export function StatusBadge({
  value,
  indicators,
}: {
  value: unknown;
  indicators?: Record<string, string>;
}) {
  if (value == null || value === "") return null;
  const label = String(value);
  const tone = (indicators?.[label] || indicators?.[label.toLowerCase()] || statusTone(label)).toLowerCase();
  return (
    <span className={`status-badge status-${tone}`}>
      <i aria-hidden="true" />
      {label}
    </span>
  );
}
