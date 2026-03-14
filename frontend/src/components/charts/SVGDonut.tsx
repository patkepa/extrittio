interface Segment {
  value: number;
  color: string;
  name: string;
}

interface SVGDonutProps {
  segments: Segment[];
  size?: number;
  strokeWidth?: number;
  children?: React.ReactNode;
}

export const SVGDonut = ({
  segments,
  size = 170,
  strokeWidth = 25,
  children,
}: SVGDonutProps) => {
  const radius = (size - strokeWidth) / 2;
  const circumference = 2 * Math.PI * radius;
  const total = segments.reduce((sum, s) => sum + s.value, 0);
  const center = size / 2;

  // 3px gap between segments as a fraction of circumference
  const gapLength = total > 0 ? 3 : 0;

  let offset = -circumference / 4; // start at 12 o'clock

  return (
    <div style={{ position: 'relative', width: size, height: size }}>
      <svg width={size} height={size} viewBox={`0 0 ${size} ${size}`}>
        {total === 0 ? (
          <circle
            cx={center}
            cy={center}
            r={radius}
            fill="none"
            stroke="rgba(255,255,255,0.08)"
            strokeWidth={strokeWidth}
          />
        ) : (
          segments.map((seg) => {
            const fraction = seg.value / total;
            const arcLength = fraction * circumference - gapLength;
            const el = (
              <circle
                key={seg.name}
                cx={center}
                cy={center}
                r={radius}
                fill="none"
                stroke={seg.color}
                strokeWidth={strokeWidth}
                strokeDasharray={`${Math.max(0, arcLength)} ${circumference - Math.max(0, arcLength)}`}
                strokeDashoffset={-offset}
                strokeLinecap="round"
              />
            );
            offset += fraction * circumference;
            return el;
          })
        )}
      </svg>
      {children && (
        <div
          style={{
            position: 'absolute',
            inset: 0,
            display: 'flex',
            flexDirection: 'column',
            alignItems: 'center',
            justifyContent: 'center',
          }}
        >
          {children}
        </div>
      )}
    </div>
  );
};
