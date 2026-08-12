import { H4, Icon } from "@blueprintjs/core";
import type { IconName } from "@blueprintjs/icons";

export interface EmptyStateProps {
  icon: IconName;
  title: string;
  description?: string;
  className?: string;
}

export const EmptyState = ({
  icon,
  title,
  description,
  className,
}: EmptyStateProps) => {
  const classNames = ["empty-state", className].filter(Boolean).join(" ");

  return (
    <div className={classNames}>
      <Icon icon={icon} size={48} />
      <H4>{title}</H4>
      {description && <p>{description}</p>}
    </div>
  );
};
