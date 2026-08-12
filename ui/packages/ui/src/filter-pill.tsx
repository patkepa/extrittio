import { Icon } from "@blueprintjs/core";
import type { IconName } from "@blueprintjs/icons";
import { StatusLed } from "./status-led";

export interface FilterPillProps<TValue extends string = string> {
  value: TValue;
  label: string;
  active?: boolean;
  count?: number;
  icon?: IconName;
  iconClassName?: string;
  status?: string;
  className?: string;
  onSelect: (value: TValue) => void;
}

export const FilterPill = <TValue extends string = string>({
  value,
  label,
  active = false,
  count,
  icon,
  iconClassName,
  status,
  className,
  onSelect,
}: FilterPillProps<TValue>) => {
  const classNames = ["filter-pill", active && "active", className]
    .filter(Boolean)
    .join(" ");

  return (
    <button className={classNames} onClick={() => onSelect(value)}>
      {status && <StatusLed status={status} />}
      {icon && <Icon icon={icon} size={12} className={iconClassName} />}
      <span className="pill-label">{label}</span>
      {count !== undefined && (
        <span className="pill-count mono-data">{count}</span>
      )}
    </button>
  );
};
