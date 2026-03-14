import uPlot from 'uplot';

/**
 * Convert row-oriented data to uPlot's column-oriented AlignedData.
 * xKey is converted to unix seconds (uPlot's native format).
 */
export function toAlignedData(
  rows: Record<string, number | string | null>[],
  xKey: string,
  yKeys: string[],
): uPlot.AlignedData {
  const xs = new Float64Array(rows.length);
  const series: (Float64Array | (number | null)[])[] = yKeys.map(() => new Array(rows.length));

  for (let i = 0; i < rows.length; i++) {
    const row = rows[i]!;
    const xVal = row[xKey];
    xs[i] =
      typeof xVal === 'string'
        ? new Date(xVal).getTime() / 1000
        : typeof xVal === 'number'
          ? xVal
          : 0;

    for (let s = 0; s < yKeys.length; s++) {
      const v = row[yKeys[s]!];
      (series[s] as (number | null)[])[i] =
        v == null ? null : typeof v === 'number' ? v : parseFloat(String(v)) || null;
    }
  }

  return [xs, ...series] as uPlot.AlignedData;
}

/** Simple sparkline data: just indices as x, values as y */
export function toSparklineData(values: number[]): uPlot.AlignedData {
  const xs = new Float64Array(values.length);
  const ys = new Float64Array(values.length);
  for (let i = 0; i < values.length; i++) {
    xs[i] = i;
    ys[i] = values[i]!;
  }
  return [xs, ys];
}

/** Sparkline options — no axes, no grid, no cursor */
export function sparklineOpts(
  color: string,
  fillOpacity = 0.15,
): Omit<uPlot.Options, 'width' | 'height'> {
  return {
    cursor: { show: false },
    legend: { show: false },
    axes: [{ show: false }, { show: false }],
    scales: { x: { time: false } },
    series: [
      {},
      {
        stroke: color,
        width: 1.5,
        fill: hexToRgba(color, fillOpacity),
        points: { show: false },
      },
    ],
  };
}

function hexToRgba(hex: string, alpha: number): string {
  const r = parseInt(hex.slice(1, 3), 16);
  const g = parseInt(hex.slice(3, 5), 16);
  const b = parseInt(hex.slice(5, 7), 16);
  return `rgba(${r},${g},${b},${alpha})`;
}

/**
 * uPlot plugin: tooltip that shows value + time on hover.
 * Appends a positioned div to the chart wrapper.
 */
export function tooltipPlugin(
  formatValue: (seriesIdx: number, val: number) => string,
  formatTime: (unixSec: number) => string,
): uPlot.Plugin {
  let tooltipEl: HTMLDivElement | null = null;

  function init(u: uPlot) {
    tooltipEl = document.createElement('div');
    tooltipEl.className = 'telemetry-tooltip';
    tooltipEl.style.position = 'absolute';
    tooltipEl.style.pointerEvents = 'none';
    tooltipEl.style.display = 'none';
    u.over.appendChild(tooltipEl);
  }

  function setCursor(u: uPlot) {
    if (!tooltipEl) return;
    const idx = u.cursor.idx;
    if (idx == null) {
      tooltipEl.style.display = 'none';
      return;
    }

    // Find the first non-null series value
    let seriesIdx = -1;
    let val: number | null = null;
    for (let s = 1; s < u.series.length; s++) {
      const d = u.data[s] as (number | null)[];
      if (d[idx] != null) {
        seriesIdx = s;
        val = d[idx]!;
        break;
      }
    }

    if (val == null || seriesIdx < 0) {
      tooltipEl.style.display = 'none';
      return;
    }

    const xVal = (u.data[0] as number[])[idx]!;
    const seriesDef = u.series[seriesIdx];
    const color = seriesDef ? (seriesDef.stroke as string) : '';

    tooltipEl.innerHTML = `
      <div class="telemetry-tooltip-time">${formatTime(xVal)}</div>
      <div class="telemetry-tooltip-value" style="color:${typeof color === 'function' ? '' : color}">${formatValue(seriesIdx, val)}</div>
    `;
    tooltipEl.style.display = 'block';

    const left = u.valToPos(xVal, 'x');
    const top = u.valToPos(val, 'y');

    // Keep tooltip within chart bounds
    const tooltipW = tooltipEl.offsetWidth;
    const chartW = u.over.clientWidth;
    const adjustedLeft = left + tooltipW + 12 > chartW ? left - tooltipW - 12 : left + 12;

    tooltipEl.style.left = `${adjustedLeft}px`;
    tooltipEl.style.top = `${Math.max(0, top - 30)}px`;
  }

  return {
    hooks: {
      init,
      setCursor,
    },
  };
}
