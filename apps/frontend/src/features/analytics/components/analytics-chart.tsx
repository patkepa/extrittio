import { useMemo, useState } from 'react';
import uPlot from 'uplot';
import { UPlotChart, type UPlotXRange } from '../../../components/charts/UPlot';
import type { AnalyticsQueryResponse } from '../api/analytics-api';
import { alignAnalyticsSeries } from '../model/chart-data';

const DEVICE_COLORS = [
  '#5f9fea',
  '#a277ff',
  '#db7a3c',
  '#73bf69',
  '#e66aa0',
  '#40b7d6',
  '#e0b400',
  '#8f78dd',
];

function seriesColor(kind: string, index: number): string {
  if (kind === 'mean') return '#ff7a00';
  if (kind === 'minimum' || kind === 'maximum') return '#657488';
  return DEVICE_COLORS[index % DEVICE_COLORS.length]!;
}

interface AnalyticsChartProps {
  result: AnalyticsQueryResponse;
}

export function AnalyticsChart({ result }: AnalyticsChartProps) {
  const [zoomRange, setZoomRange] = useState<UPlotXRange | null>(null);
  const data = useMemo(() => alignAnalyticsSeries(result.series), [result.series]);
  const options = useMemo((): Omit<uPlot.Options, 'width' | 'height'> => {
    const minimumIndex = result.series.findIndex((series) => series.kind === 'minimum');
    const maximumIndex = result.series.findIndex((series) => series.kind === 'maximum');
    const unit = result.metric.unit ?? '';
    const precision = result.metric.precision ?? 1;
    const percentMetric = unit.includes('%');

    return {
      cursor: { x: true, y: false, drag: { x: false, y: false } },
      legend: { show: true, live: true },
      axes: [
        {
          stroke: 'rgba(255,255,255,0.48)',
          font: '11px system-ui',
          ticks: { stroke: 'rgba(255,255,255,0.08)', width: 1 },
          grid: { show: false },
          gap: 8,
        },
        {
          stroke: 'rgba(255,255,255,0.48)',
          font: '11px system-ui',
          ticks: { show: false },
          grid: { stroke: 'rgba(255,255,255,0.08)', width: 1, dash: [3, 3] },
          size: 58,
          values: (_chart, values) => values.map((value) => `${value}${unit}`),
        },
      ],
      scales: percentMetric ? { y: { range: () => [0, 100] } } : { y: {} },
      series: [
        {},
        ...result.series.map((series, index) => ({
          label: series.label,
          stroke: seriesColor(series.kind, index),
          width: series.kind === 'mean' ? 2 : series.kind === 'device' ? 1.5 : 1,
          dash: series.kind === 'minimum' || series.kind === 'maximum' ? [5, 4] : undefined,
          points: { show: false },
          spanGaps: false,
          value: (_chart: uPlot, value: number) =>
            value == null ? '—' : `${value.toFixed(precision)}${unit}`,
        })),
      ],
      bands:
        minimumIndex >= 0 && maximumIndex >= 0
          ? [
              {
                series: [minimumIndex + 1, maximumIndex + 1],
                fill: 'rgba(101, 116, 136, 0.14)',
              },
            ]
          : undefined,
    };
  }, [result.metric.precision, result.metric.unit, result.series]);

  return (
    <div className="analytics-chart">
      <UPlotChart
        options={options}
        data={data}
        height={360}
        zoomable
        xRange={zoomRange}
        onXRangeChange={setZoomRange}
      />
    </div>
  );
}
