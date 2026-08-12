import type { ReactNode } from "react";

export interface DemoFrameProps {
  children: ReactNode;
  title?: ReactNode;
  eyebrow?: ReactNode;
  actions?: ReactNode;
  footer?: ReactNode;
  className?: string;
}

export const DemoFrame = ({
  children,
  title,
  eyebrow,
  actions,
  footer,
  className,
}: DemoFrameProps) => {
  const classNames = ["demo-frame", className].filter(Boolean).join(" ");
  const hasHeader = eyebrow || title || actions;

  return (
    <div className={classNames}>
      {hasHeader && (
        <div className="demo-frame-header">
          <div>
            {eyebrow && <div className="demo-frame-eyebrow">{eyebrow}</div>}
            {title && <h3>{title}</h3>}
          </div>
          {actions && <div className="demo-frame-actions">{actions}</div>}
        </div>
      )}
      <div className="demo-frame-body">{children}</div>
      {footer && <div className="demo-frame-footer">{footer}</div>}
    </div>
  );
};
