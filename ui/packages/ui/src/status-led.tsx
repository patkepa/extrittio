export interface StatusLedProps {
  status: string;
  className?: string;
  label?: string;
}

export const StatusLed = ({ status, className, label }: StatusLedProps) => {
  const classNames = ["status-led", `status-led--${status}`, className]
    .filter(Boolean)
    .join(" ");

  return (
    <span
      className={classNames}
      aria-hidden={label ? undefined : true}
      aria-label={label}
      role={label ? "img" : undefined}
    />
  );
};
