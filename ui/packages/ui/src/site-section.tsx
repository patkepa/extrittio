import type { ReactNode } from "react";

export interface SiteSectionProps {
  children: ReactNode;
  id?: string;
  eyebrow?: ReactNode;
  title?: ReactNode;
  description?: ReactNode;
  actions?: ReactNode;
  className?: string;
  contentClassName?: string;
}

export const SiteSection = ({
  children,
  id,
  eyebrow,
  title,
  description,
  actions,
  className,
  contentClassName,
}: SiteSectionProps) => {
  const sectionClassNames = ["site-section", className]
    .filter(Boolean)
    .join(" ");
  const contentClassNames = ["site-section-content", contentClassName]
    .filter(Boolean)
    .join(" ");
  const hasHeader = eyebrow || title || description || actions;

  return (
    <section className={sectionClassNames} id={id}>
      <div className={contentClassNames}>
        {hasHeader && (
          <div className="site-section-header">
            <div>
              {eyebrow && <div className="site-section-eyebrow">{eyebrow}</div>}
              {title && <h2 className="site-section-title">{title}</h2>}
              {description && (
                <div className="site-section-description">{description}</div>
              )}
            </div>
            {actions && <div className="site-section-actions">{actions}</div>}
          </div>
        )}
        {children}
      </div>
    </section>
  );
};
