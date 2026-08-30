export function Chart({
  kind,
  series,
  onSegmentClick,
}: {
  kind?: string;
  series: Array<{ label: string; value: number }>;
  onSegmentClick?: (label: string) => void;
}) {
  if (!series.length) return <p className="muted">No data</p>;
  const max = Math.max(...series.map((s) => s.value), 1);
  if (kind === "pie" || kind === "donut") {
    const total = series.reduce((s, x) => s + x.value, 0) || 1;
    let acc = 0;
    const cx = 50;
    const cy = 50;
    const r = 36;
    const inner = kind === "donut" ? 20 : 0;
    return (
      <svg viewBox="0 0 100 100" className="chart" role="img">
        {series.map((s, i) => {
          const start = acc / total;
          acc += s.value;
          const end = acc / total;
          const path = arc(cx, cy, r, start, end);
          return (
            <path
              key={s.label}
              d={path}
              fill={color(i)}
              role={onSegmentClick ? "button" : undefined}
              tabIndex={onSegmentClick ? 0 : undefined}
              onClick={() => onSegmentClick?.(s.label)}
              style={{ cursor: onSegmentClick ? "pointer" : undefined }}
            />
          );
        })}
        {inner ? <circle cx={cx} cy={cy} r={inner} fill="var(--panel)" /> : null}
      </svg>
    );
  }
  if (kind === "line" || kind === "area") {
    const pts = series
      .map((s, i) => {
        const x = (i / Math.max(series.length - 1, 1)) * 100;
        const y = 100 - (s.value / max) * 90;
        return `${x},${y}`;
      })
      .join(" ");
    const area = `0,100 ${pts} 100,100`;
    return (
      <svg viewBox="0 0 100 100" className="chart" preserveAspectRatio="none" role="img">
        {kind === "area" ? <polygon fill="var(--accent)" fillOpacity="0.2" points={area} /> : null}
        <polyline fill="none" stroke="var(--accent)" strokeWidth="2" points={pts} />
      </svg>
    );
  }
  return (
    <div className="bar-chart" role="img">
      {series.map((s, i) => {
        const inner = (
          <>
            <span className="muted">{s.label}</span>
            <div className="bar-track">
              <div className="bar-fill" style={{ width: `${(s.value / max) * 100}%`, background: color(i) }} />
            </div>
            <strong>{s.value}</strong>
          </>
        );
        return onSegmentClick ? (
          <button key={s.label} type="button" className="bar-row" onClick={() => onSegmentClick(s.label)}>
            {inner}
          </button>
        ) : (
          <div key={s.label} className="bar-row">
            {inner}
          </div>
        );
      })}
    </div>
  );
}

function arc(cx: number, cy: number, r: number, start: number, end: number) {
  const a0 = start * 2 * Math.PI - Math.PI / 2;
  const a1 = end * 2 * Math.PI - Math.PI / 2;
  const x0 = cx + r * Math.cos(a0);
  const y0 = cy + r * Math.sin(a0);
  const x1 = cx + r * Math.cos(a1);
  const y1 = cy + r * Math.sin(a1);
  const large = end - start > 0.5 ? 1 : 0;
  return `M ${cx} ${cy} L ${x0} ${y0} A ${r} ${r} 0 ${large} 1 ${x1} ${y1} Z`;
}

function color(i: number) {
  const palette = ["#2563eb", "#059669", "#d97706", "#dc2626", "#7c3aed", "#0891b2"];
  return palette[i % palette.length];
}
