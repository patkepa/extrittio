import { useMemo } from "react";
import { UPlotChart } from "../charts/UPlot";
import { toSparklineData, sparklineOpts } from "../charts/uplot-helpers";

interface MetricSparklineProps {
  data: number[];
  color: string;
  height?: number;
}

export const MetricSparkline = ({
  data,
  color,
  height = 32,
}: MetricSparklineProps) => {
  const plotData = useMemo(() => toSparklineData(data), [data]);
  const opts = useMemo(() => sparklineOpts(color, 0.3), [color]);

  return <UPlotChart options={opts} data={plotData} height={height} />;
};
