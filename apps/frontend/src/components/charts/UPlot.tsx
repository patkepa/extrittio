import { useRef, useEffect } from 'react';
import uPlot from 'uplot';
import 'uplot/dist/uPlot.min.css';

interface UPlotProps {
  options: Omit<uPlot.Options, 'width' | 'height'>;
  data: uPlot.AlignedData;
  width?: number;
  height?: number;
}

export const UPlotChart = ({ options, data, width, height }: UPlotProps) => {
  const containerRef = useRef<HTMLDivElement>(null);
  const chartRef = useRef<uPlot | null>(null);

  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;

    const ro = new ResizeObserver((entries) => {
      const entry = entries[0];
      if (!entry) return;
      const w = width ?? entry.contentRect.width;
      const h = height ?? entry.contentRect.height;
      if (w === 0 || h === 0) return;

      if (chartRef.current) {
        chartRef.current.setSize({ width: w, height: h });
      } else {
        chartRef.current = new uPlot({ ...options, width: w, height: h }, data, el);
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
  }, [options, height, width]);

  // Update data without remounting
  useEffect(() => {
    if (chartRef.current) {
      chartRef.current.setData(data);
    }
  }, [data]);

  return <div ref={containerRef} style={{ width: '100%', height: height ?? '100%' }} />;
};
