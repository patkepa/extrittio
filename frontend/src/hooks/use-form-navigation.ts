import { RefObject, useEffect } from 'react';
import { getDirectionalKey, isEditableTarget } from '../utils/keyboard';

const FOCUSABLE_SELECTOR = [
  'input:not([disabled])',
  'select:not([disabled])',
  'textarea:not([disabled])',
  'button:not([disabled])',
  'a[href]',
  '[tabindex]:not([tabindex="-1"])',
].join(',');

function getFocusableControls(container: HTMLElement): HTMLElement[] {
  return Array.from(container.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR)).filter(
    (element) => element.offsetParent !== null && !element.getAttribute('aria-hidden'),
  );
}

function allowsArrowEscapeFromEditable(event: KeyboardEvent): boolean {
  if (!isEditableTarget(event.target)) return true;
  if (!(event.target instanceof HTMLInputElement)) return false;
  return event.key === 'ArrowUp' || event.key === 'ArrowDown';
}

export function useFormNavigation(containerRef: RefObject<HTMLElement>, enabled = true) {
  useEffect(() => {
    if (!enabled) return;

    const container = containerRef.current;
    if (!container) return;

    const handleKeyDown = (event: KeyboardEvent) => {
      if (
        event.defaultPrevented ||
        event.isComposing ||
        event.metaKey ||
        event.ctrlKey ||
        event.altKey ||
        !allowsArrowEscapeFromEditable(event)
      ) {
        return;
      }

      const direction = getDirectionalKey(event);
      if (!direction) return;

      const controls = getFocusableControls(container);
      if (controls.length === 0) return;

      const activeElement = document.activeElement;
      const currentIndex =
        activeElement instanceof HTMLElement ? controls.indexOf(activeElement) : -1;
      if (currentIndex === -1) return;

      let nextIndex = currentIndex;
      if (direction === 'down' || direction === 'right') nextIndex = currentIndex + 1;
      if (direction === 'up' || direction === 'left') nextIndex = currentIndex - 1;
      if (direction === 'first') nextIndex = 0;
      if (direction === 'last') nextIndex = controls.length - 1;

      nextIndex = Math.min(Math.max(nextIndex, 0), controls.length - 1);
      if (nextIndex === currentIndex) return;

      event.preventDefault();
      controls[nextIndex]?.focus();
    };

    container.addEventListener('keydown', handleKeyDown);
    return () => container.removeEventListener('keydown', handleKeyDown);
  }, [containerRef, enabled]);
}
