import { useEffect, useState } from "react";
import { api } from "../../api";
import { Chart } from "../dashboards/Chart";
import { EmptyState, ErrorState, Skeleton } from "../ui/EmptyState";
import { groupingField } from "../../metadata/views";
import type { CollectionViewProps } from "../../views/registry";

export default function ChartView({ meta, slug, onError }: CollectionViewProps) {
  const spec = meta.views?.chart;
  const dimension = spec?.dimension || groupingField(meta)?.name || "status";
  const aggregation = spec?.measure?.aggregation || "count";
  const field = spec?.measure?.field;
  const kind = spec?.type || "bar";
  const [series, setSeries] = useState<Array<{ label: string; value: number }>>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");

  useEffect(() => {
    const params = new URLSearchParams();
    params.set("group_by", dimension);
    params.set("metric", aggregation);
    if (field) params.set("field", field);
    setLoading(true);
    api
      .aggregates(slug, params)
      .then((d) => {
        setSeries(d.series ?? []);
        setError("");
      })
      .catch((e) => {
        const message = e instanceof Error ? e.message : "Unable to load chart";
        setError(message);
        onError(message);
      })
      .finally(() => setLoading(false));
  }, [slug, dimension, aggregation, field, onError]);

  if (loading) return <Skeleton rows={3} />;
  if (error) return <ErrorState message={error} />;
  if (!series.length) {
    return <EmptyState title="No chart data" description="Nothing matches the current filters." />;
  }
  return (
    <div className="panel chart-view">
      <Chart kind={kind} series={series} />
    </div>
  );
}
