export interface MetricStripItem {
  label: string;
  value: string;
  description?: string;
}

export interface MetricStripProps {
  metrics: MetricStripItem[];
  className?: string;
}

export const MetricStrip = ({ metrics, className }: MetricStripProps) => {
  const classNames = ["metric-strip", className].filter(Boolean).join(" ");

  return (
    <dl className={classNames}>
      {metrics.map((metric) => (
        <div className="metric-strip-item" key={metric.label}>
          <dt>{metric.label}</dt>
          <dd className="mono-data">{metric.value}</dd>
          {metric.description && <p>{metric.description}</p>}
        </div>
      ))}
    </dl>
  );
};
