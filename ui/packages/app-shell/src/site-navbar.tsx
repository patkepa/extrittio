import {
  useEffect,
  useState,
  type CSSProperties,
  type MouseEvent,
  type ReactNode,
} from "react";
import {
  Button,
  Popover,
  PopoverInteractionKind,
  Position,
} from "@blueprintjs/core";
import type { SiteNavAction, SiteNavItem } from "@extrittio/navigation";
import { WorkspaceNavbar } from "./workspace-navbar";

export interface SiteNavbarProps {
  productName: string;
  homeHref?: string;
  navItems?: SiteNavItem[];
  actions?: SiteNavAction[];
  utilities?: ReactNode;
  currentPath?: string;
  className?: string;
  brand?: ReactNode;
  onNavigate?: (href: string) => void;
}

type GroupedMenuSectionStyle = CSSProperties & {
  "--site-navbar-menu-section-rows": number;
  "--site-navbar-menu-section-items": number;
};

function isActiveHref(href: string, currentPath?: string) {
  if (!currentPath) return false;
  if (href === "/") return currentPath === "/";
  return currentPath === href || currentPath.startsWith(`${href}/`);
}

function getLinkTarget(external?: boolean) {
  return external ? "_blank" : undefined;
}

function getLinkRel(external?: boolean) {
  return external ? "noreferrer" : undefined;
}

export const SiteNavbar = ({
  productName,
  homeHref = "/",
  navItems = [],
  actions = [],
  utilities,
  currentPath,
  className,
  brand,
  onNavigate,
}: SiteNavbarProps) => {
  const [mobileOpen, setMobileOpen] = useState(false);
  const [openDesktopMenu, setOpenDesktopMenu] = useState<string | null>(null);

  const classNames = ["site-navbar", className].filter(Boolean).join(" ");

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setMobileOpen(false);
        setOpenDesktopMenu(null);
      }
    };

    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, []);

  useEffect(() => {
    const closeDesktopMenu = () => setOpenDesktopMenu(null);

    window.addEventListener("scroll", closeDesktopMenu, {
      capture: true,
      passive: true,
    });
    return () => {
      window.removeEventListener("scroll", closeDesktopMenu, {
        capture: true,
      });
    };
  }, []);

  const handleNavigate =
    (href: string, external?: boolean) => (event: MouseEvent<HTMLElement>) => {
      if (external) return;
      setMobileOpen(false);
      setOpenDesktopMenu(null);
      if (!onNavigate) return;

      event.preventDefault();
      onNavigate(href);
    };

  function hasActiveChild(item: SiteNavItem): boolean {
    return (
      item.children?.some(
        (child) =>
          isActiveHref(child.href, currentPath) || hasActiveChild(child),
      ) ?? false
    );
  }

  const renderDesktopMenuLink = (child: SiteNavItem) => {
    const active =
      isActiveHref(child.href, currentPath) || hasActiveChild(child);

    return (
      <a
        key={`${child.label}-${child.href}`}
        className={[
          "site-navbar-menu-item",
          active && "site-navbar-menu-item--active",
        ]
          .filter(Boolean)
          .join(" ")}
        role="menuitem"
        href={child.href}
        target={getLinkTarget(child.external)}
        rel={getLinkRel(child.external)}
        aria-current={
          isActiveHref(child.href, currentPath) ? "page" : undefined
        }
        onClick={handleNavigate(child.href, child.external)}
      >
        <span className="site-navbar-menu-copy">
          <span>{child.label}</span>
          {child.description && <small>{child.description}</small>}
        </span>
      </a>
    );
  };

  const renderDesktopNavItem = (item: SiteNavItem) => {
    const active = isActiveHref(item.href, currentPath) || hasActiveChild(item);
    const hasChildren = Boolean(item.children?.length);

    if (hasChildren) {
      const menuId = `${item.label}-${item.href}`;
      const grouped = item.children!.some((child) =>
        Boolean(child.children?.length),
      );
      const maxGroupedChildCount = grouped
        ? Math.max(
            ...item.children!.map((child) => child.children?.length ?? 1),
          )
        : 0;
      const getGroupedMenuSectionStyle = (
        itemCount: number,
      ): GroupedMenuSectionStyle => ({
        "--site-navbar-menu-section-rows": maxGroupedChildCount + 1,
        "--site-navbar-menu-section-items": itemCount,
      });

      return (
        <Popover
          key={item.label}
          isOpen={openDesktopMenu === menuId}
          minimal
          interactionKind={PopoverInteractionKind.HOVER}
          hoverOpenDelay={45}
          hoverCloseDelay={140}
          onInteraction={(nextOpenState) =>
            setOpenDesktopMenu(nextOpenState ? menuId : null)
          }
          openOnTargetFocus
          position={Position.BOTTOM_LEFT}
          popoverClassName="site-navbar-popover"
          content={
            <div
              className={[
                "site-navbar-menu",
                grouped && "site-navbar-menu--grouped",
              ]
                .filter(Boolean)
                .join(" ")}
              role="menu"
              aria-label={item.label}
            >
              {item.children!.map((child) =>
                child.children?.length ? (
                  <section
                    key={`${child.label}-${child.href}`}
                    className="site-navbar-menu-section"
                    style={getGroupedMenuSectionStyle(child.children.length)}
                    role="none"
                  >
                    <div className="site-navbar-menu-section-title">
                      {child.label}
                    </div>
                    <div className="site-navbar-menu-section-items">
                      {child.children.map((nestedChild) =>
                        renderDesktopMenuLink(nestedChild),
                      )}
                    </div>
                  </section>
                ) : (
                  renderDesktopMenuLink(child)
                ),
              )}
            </div>
          }
        >
          <Button
            className={[
              "site-navbar-link",
              active && "site-navbar-link--active",
            ]
              .filter(Boolean)
              .join(" ")}
            minimal
            text={item.label}
          />
        </Popover>
      );
    }

    return (
      <a
        key={item.label}
        className={["site-navbar-link", active && "site-navbar-link--active"]
          .filter(Boolean)
          .join(" ")}
        href={item.href}
        target={getLinkTarget(item.external)}
        rel={getLinkRel(item.external)}
        onClick={handleNavigate(item.href, item.external)}
      >
        {item.label}
      </a>
    );
  };

  const renderMobileNavItem = (item: SiteNavItem) => {
    const active = isActiveHref(item.href, currentPath) || hasActiveChild(item);
    const hasChildren = Boolean(item.children?.length);

    if (hasChildren) {
      return (
        <details
          className="site-mobile-nav-group"
          key={item.label}
          open={active || undefined}
        >
          <summary>{item.label}</summary>
          <div className="site-mobile-nav-children">
            {item.children!.map((child) => renderMobileNavItem(child))}
          </div>
        </details>
      );
    }

    return (
      <a
        key={item.label}
        className={[
          "site-mobile-nav-link",
          active && "site-mobile-nav-link--active",
        ]
          .filter(Boolean)
          .join(" ")}
        href={item.href}
        target={getLinkTarget(item.external)}
        rel={getLinkRel(item.external)}
        onClick={handleNavigate(item.href, item.external)}
      >
        {item.label}
      </a>
    );
  };

  const renderAction = (action: SiteNavAction, compact = false) => (
    <a
      key={action.label}
      className={[
        compact ? "site-mobile-action" : "site-navbar-action",
        action.intent === "primary" && "site-navbar-action--primary",
      ]
        .filter(Boolean)
        .join(" ")}
      href={action.href}
      target={getLinkTarget(action.external)}
      rel={getLinkRel(action.external)}
      onClick={handleNavigate(action.href, action.external)}
    >
      {action.label}
    </a>
  );

  return (
    <WorkspaceNavbar
      className={classNames}
      barClassName="site-navbar-bar"
      left={
        <div className="site-navbar-left">
          <a
            className="site-navbar-brand"
            href={homeHref}
            onClick={handleNavigate(homeHref)}
          >
            {brand ?? (
              <span className="site-navbar-brand-text">{productName}</span>
            )}
          </a>

          {navItems.length > 0 && (
            <nav className="site-navbar-links" aria-label="Primary navigation">
              {navItems.map((item) => renderDesktopNavItem(item))}
            </nav>
          )}
        </div>
      }
      right={
        <div className="site-navbar-right">
          <div className="site-navbar-actions">
            {actions.map((action) => renderAction(action))}
          </div>

          {(navItems.length > 0 || actions.length > 0) && (
            <Button
              className="site-navbar-menu-button"
              icon={mobileOpen ? "cross" : "menu"}
              minimal
              aria-label={mobileOpen ? "Close navigation" : "Open navigation"}
              aria-expanded={mobileOpen}
              onClick={() => setMobileOpen((open) => !open)}
            />
          )}

          {utilities && (
            <div className="site-navbar-utilities">{utilities}</div>
          )}
        </div>
      }
      expanded={mobileOpen}
      expandedContent={
        <div className="site-mobile-nav">
          <nav aria-label="Mobile navigation">
            {navItems.map((item) => renderMobileNavItem(item))}
          </nav>
          {actions.length > 0 && (
            <div className="site-mobile-actions">
              {actions.map((action) => renderAction(action, true))}
            </div>
          )}
        </div>
      }
    />
  );
};
