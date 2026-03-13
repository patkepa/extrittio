import { AreaChart, Area, ResponsiveContainer } from "recharts";

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
  const chartData = data.map((v, i) => ({ v, i }));
  const gradientId = `sparkline-${color.replace("#", "")}`;

  return (
    <ResponsiveContainer width="100%" height={height}>
      <AreaChart data={chartData}>
        <defs>
          <linearGradient id={gradientId} x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stopColor={color} stopOpacity={0.3} />
            <stop offset="100%" stopColor={color} stopOpacity={0} />
          </linearGradient>
        </defs>
        <Area
          type="monotone"
          dataKey="v"
          stroke={color}
          strokeWidth={1.5}
          fill={`url(#${gradientId})`}
          dot={false}
          isAnimationActive={false}
        />
      </AreaChart>
    </ResponsiveContainer>
  );
};
