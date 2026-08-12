import type { ReactNode } from "react";

export interface CtaBarProps {
  title: ReactNode;
  description?: ReactNode;
  actions?: ReactNode;
  className?: string;
}

export const CtaBar = ({
  title,
  description,
  actions,
  className,
}: CtaBarProps) => {
  const classNames = ["cta-bar", className].filter(Boolean).join(" ");

  return (
    <section className={classNames}>
      <div className="cta-bar-copy">
        <h2>{title}</h2>
        {description && (
          <div className="cta-bar-description">{description}</div>
        )}
      </div>
      {actions && <div className="cta-bar-actions">{actions}</div>}
    </section>
  );
};
