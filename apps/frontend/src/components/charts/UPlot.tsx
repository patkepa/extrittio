import { useRef, useEffect } from 'react';
import uPlot from 'uplot';
import 'uplot/dist/uPlot.min.css';
import './UPlot.css';

export interface UPlotXRange {
  min: number;
  max: number;
}

interface UPlotProps {
  options: Omit<uPlot.Options, 'width' | 'height'>;
  data: uPlot.AlignedData;
  width?: number;
  height?: number;
  zoomable?: boolean;
  xRange?: UPlotXRange | null;
  onXRangeChange?: (range: UPlotXRange | null) => void;
}

const MIN_ZOOM_DRAG_PX = 6;

export const UPlotChart = ({
  options,
  data,
  width,
  height,
  zoomable = false,
  xRange = null,
  onXRangeChange,
}: UPlotProps) => {
  const containerRef = useRef<HTMLDivElement>(null);
  const chartRef = useRef<uPlot | null>(null);
  const xRangeRef = useRef(xRange);
  const onXRangeChangeRef = useRef(onXRangeChange);

  xRangeRef.current = xRange;
  onXRangeChangeRef.current = onXRangeChange;

  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;

    const zoomPlugin: uPlot.Plugin = {
      hooks: {
        setSelect: (chart) => {
          const { left, width: selectionWidth } = chart.select;
          if (selectionWidth < MIN_ZOOM_DRAG_PX) return;

          const start = chart.posToVal(left, 'x');
          const end = chart.posToVal(left + selectionWidth, 'x');
          const min = Math.min(start, end);
          const max = Math.max(start, end);
          if (!Number.isFinite(min) || !Number.isFinite(max) || min === max) return;

          onXRangeChangeRef.current?.({ min, max });
        },
      },
    };

    const chartOptions: uPlot.Options = {
      ...options,
      width: 0,
      height: 0,
      cursor: zoomable
        ? {
            ...options.cursor,
            drag: {
              ...options.cursor?.drag,
              x: true,
              y: false,
              setScale: true,
              dist: MIN_ZOOM_DRAG_PX,
            },
          }
        : options.cursor,
      plugins: zoomable ? [...(options.plugins ?? []), zoomPlugin] : options.plugins,
    };

    const ro = new ResizeObserver((entries) => {
      const entry = entries[0];
      if (!entry) return;
      const w = width ?? entry.contentRect.width;
      const h = height ?? entry.contentRect.height;
      if (w === 0 || h === 0) return;

      if (chartRef.current) {
        chartRef.current.setSize({ width: w, height: h });
      } else {
        const chart = new uPlot({ ...chartOptions, width: w, height: h }, data, el);
        chartRef.current = chart;

        const range = xRangeRef.current;
        if (zoomable && range) {
          chart.setScale('x', range);
        }
      }
    });

    ro.observe(el);

    return () => {
      ro.disconnect();
      chartRef.current?.destroy();
      chartRef.current = null;
    };
    // Only remount when options identity changes
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [options, height, width, zoomable]);

  // Update data without remounting. Preserve a selected time window during refreshes.
  useEffect(() => {
    const chart = chartRef.current;
    if (!chart) return;

    chart.setData(data, !zoomable || xRange == null);
    if (zoomable && xRange) {
      chart.setScale('x', xRange);
    }
  }, [data, xRange, zoomable]);

  return (
    <div
      className={zoomable ? 'uplot-chart uplot-chart--zoomable' : 'uplot-chart'}
      title={zoomable ? 'Drag horizontally to zoom. Double-click to reset.' : undefined}
      onDoubleClick={zoomable ? () => onXRangeChangeRef.current?.(null) : undefined}
    >
      <div ref={containerRef} style={{ width: '100%', height: height ?? '100%' }} />
    </div>
  );
};
