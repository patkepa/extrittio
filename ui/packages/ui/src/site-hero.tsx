import type { CSSProperties, ReactNode } from "react";

export interface SiteHeroProps {
  title: ReactNode;
  id?: string;
  eyebrow?: ReactNode;
  description?: ReactNode;
  actions?: ReactNode;
  media?: ReactNode;
  children?: ReactNode;
  className?: string;
  style?: CSSProperties;
}

export const SiteHero = ({
  title,
  id,
  eyebrow,
  description,
  actions,
  media,
  children,
  className,
  style,
}: SiteHeroProps) => {
  const classNames = ["site-hero", media && "site-hero--with-media", className]
    .filter(Boolean)
    .join(" ");

  return (
    <section className={classNames} id={id} style={style}>
      <div className="site-hero-inner">
        <div className="site-hero-copy">
          {eyebrow && <div className="site-hero-eyebrow">{eyebrow}</div>}
          <h1 className="site-hero-title">{title}</h1>
          {description && (
            <div className="site-hero-description">{description}</div>
          )}
          {actions && <div className="site-hero-actions">{actions}</div>}
          {children}
        </div>
        {media && <div className="site-hero-media">{media}</div>}
      </div>
    </section>
  );
};
