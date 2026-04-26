import { useRef } from 'react';
import type { CSSProperties, KeyboardEvent, MouseEvent, ReactNode } from 'react';
import { getDirectionalKey, isEditableTarget } from '../../utils/keyboard';
import './right-sidebar.css';

interface RightSidebarProps {
  children: ReactNode;
  collapsed?: boolean;
  className?: string;
  mode?: 'inline' | 'overlay';
  width?: number | string;
  ariaLabel?: string;
}

const RIGHT_SIDEBAR_ITEM_SELECTOR = '[data-right-sidebar-item="true"]';
const RIGHT_SIDEBAR_FOCUSABLE_SELECTOR = [
  RIGHT_SIDEBAR_ITEM_SELECTOR,
  'button:not([disabled])',
  'a[href]',
  'input:not([disabled])',
  'select:not([disabled])',
  'textarea:not([disabled])',
  '[tabindex]:not([tabindex="-1"])',
].join(',');

const isActivationKey = (key: string) => key === 'Enter' || key === ' ' || key === 'Spacebar';

function isVisibleElement(element: HTMLElement): boolean {
  if (element.getAttribute('aria-hidden') === 'true') return false;
  if (element.closest('[aria-hidden="true"]')) return false;

  const style = window.getComputedStyle(element);
  return (
    style.display !== 'none' &&
    style.visibility !== 'hidden' &&
    style.opacity !== '0' &&
    element.getClientRects().length > 0
  );
}

function getNavigableElements(sidebar: HTMLElement): HTMLElement[] {
  const elements = Array.from(
    sidebar.querySelectorAll<HTMLElement>(RIGHT_SIDEBAR_FOCUSABLE_SELECTOR),
  ).filter((element) => isVisibleElement(element));

  return Array.from(new Set(elements));
}

function allowsArrowEscapeFromEditable(event: KeyboardEvent<HTMLElement>): boolean {
  if (!isEditableTarget(event.target)) return true;
  if (!(event.target instanceof HTMLInputElement)) return false;
  return event.key === 'ArrowUp' || event.key === 'ArrowDown';
}

export const RightSidebar = ({
  children,
  collapsed = false,
  className,
  mode = 'inline',
  width,
  ariaLabel,
}: RightSidebarProps) => {
  const sidebarRef = useRef<HTMLElement | null>(null);

  const style =
    width === undefined
      ? undefined
      : ({
          '--right-sidebar-width': typeof width === 'number' ? `${width}px` : width,
        } as CSSProperties);

  const focusElement = (index: number) => {
    const sidebar = sidebarRef.current;
    if (!sidebar) return;

    const elements = getNavigableElements(sidebar);
    if (elements.length === 0) {
      sidebar.focus();
      return;
    }

    const nextIndex = Math.min(Math.max(index, 0), elements.length - 1);
    const nextElement = elements[nextIndex];
    nextElement?.focus({ preventScroll: true });
    nextElement?.scrollIntoView({ block: 'nearest', inline: 'nearest' });
  };

  const focusInitialElement = () => {
    const sidebar = sidebarRef.current;
    if (!sidebar) return;

    const elements = getNavigableElements(sidebar);
    const initialIndex = elements.findIndex(
      (element) => element.dataset.focusRegionInitial === 'true',
    );
    focusElement(initialIndex >= 0 ? initialIndex : 0);
  };

  const handleMouseDown = (event: MouseEvent<HTMLElement>) => {
    sidebarRef.current?.removeAttribute('data-focus-region-keyboard');

    if (!(event.target instanceof HTMLElement)) return;

    if (event.target.closest('button, a, input, select, textarea')) return;

    const item = event.target.closest<HTMLElement>(RIGHT_SIDEBAR_ITEM_SELECTOR);
    if (item) {
      item.focus();
      return;
    }

    sidebarRef.current?.focus();
  };

  const handleKeyDown = (event: KeyboardEvent<HTMLElement>) => {
    if (
      event.defaultPrevented ||
      event.nativeEvent.isComposing ||
      event.shiftKey ||
      !allowsArrowEscapeFromEditable(event)
    ) {
      return;
    }

    const sidebar = sidebarRef.current;
    if (!sidebar) return;

    const direction = getDirectionalKey(event);
    const canActivate = isActivationKey(event.key);
    if (!direction && !canActivate) return;

    const elements = getNavigableElements(sidebar);
    if (elements.length === 0) return;

    const target = event.target instanceof HTMLElement ? event.target : null;
    const currentElement = target
      ? (elements.find((element) => element === target) ??
        target.closest<HTMLElement>(RIGHT_SIDEBAR_ITEM_SELECTOR))
      : null;

    if (
      canActivate &&
      currentElement instanceof HTMLElement &&
      currentElement.matches(RIGHT_SIDEBAR_ITEM_SELECTOR) &&
      currentElement === target
    ) {
      event.preventDefault();
      currentElement.click();
      return;
    }

    if (!direction) return;

    const currentIndex = currentElement ? elements.indexOf(currentElement) : -1;
    if (currentIndex === -1) {
      event.preventDefault();
      focusInitialElement();
      return;
    }

    let nextIndex = currentIndex;
    if (direction === 'down' || direction === 'right') nextIndex = currentIndex + 1;
    if (direction === 'up' || direction === 'left') nextIndex = currentIndex - 1;
    if (direction === 'first') nextIndex = 0;
    if (direction === 'last') nextIndex = elements.length - 1;

    nextIndex = Math.min(Math.max(nextIndex, 0), elements.length - 1);
    if (nextIndex === currentIndex) return;

    event.preventDefault();
    focusElement(nextIndex);
  };

  return (
    <aside
      ref={sidebarRef}
      className={[
        'right-sidebar',
        `right-sidebar--${mode}`,
        collapsed && 'right-sidebar--collapsed',
        className,
      ]
        .filter(Boolean)
        .join(' ')}
      style={style}
      data-focus-region={collapsed ? undefined : 'aside'}
      tabIndex={collapsed ? undefined : -1}
      aria-hidden={collapsed || undefined}
      aria-label={ariaLabel}
      onKeyDown={handleKeyDown}
      onMouseDown={handleMouseDown}
    >
      {children}
    </aside>
  );
};
