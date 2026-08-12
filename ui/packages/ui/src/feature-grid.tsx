import { Icon } from "@blueprintjs/core";
import type { IconName } from "@blueprintjs/icons";
import { SiteGrid } from "./site-grid";

export interface FeatureGridItem {
  title: string;
  description: string;
  icon?: IconName;
  meta?: string;
}

export interface FeatureGridProps {
  features: FeatureGridItem[];
  className?: string;
}

export const FeatureGrid = ({ features, className }: FeatureGridProps) => {
  const classNames = ["feature-grid", className].filter(Boolean).join(" ");

  return (
    <SiteGrid className={classNames} minColumnWidth="260px">
      {features.map((feature) => (
        <article className="feature-grid-item" key={feature.title}>
          <div className="feature-grid-item-header">
            {feature.icon && (
              <span className="feature-grid-icon" aria-hidden="true">
                <Icon icon={feature.icon} size={16} />
              </span>
            )}
            {feature.meta && (
              <span className="feature-grid-meta mono-data">
                {feature.meta}
              </span>
            )}
          </div>
          <h3>{feature.title}</h3>
          <p>{feature.description}</p>
        </article>
      ))}
    </SiteGrid>
  );
};
