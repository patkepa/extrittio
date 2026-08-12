import type { KeyboardEvent as ReactKeyboardEvent } from "react";

export type DirectionalKey =
  | "up"
  | "down"
  | "left"
  | "right"
  | "first"
  | "last";

export function isEditableTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;

  if (target.isContentEditable) return true;

  const editable = target.closest(
    'input, textarea, select, [contenteditable="true"], [role="textbox"], [role="searchbox"], [role="combobox"], [role="spinbutton"]',
  );

  return editable instanceof HTMLElement;
}

export function hasOpenBlockingOverlay(): boolean {
  return Boolean(
    document.querySelector(
      '.bp6-overlay-open .bp6-dialog, .bp6-overlay-open [role="dialog"], [cmdk-dialog]',
    ),
  );
}

export function getDirectionalKey(
  event: KeyboardEvent | ReactKeyboardEvent,
): DirectionalKey | null {
  if (event.shiftKey) return null;

  switch (event.key) {
    case "ArrowUp":
      return "up";
    case "ArrowDown":
      return "down";
    case "ArrowLeft":
      return "left";
    case "ArrowRight":
      return "right";
    case "Home":
      return "first";
    case "End":
      return "last";
    default:
      break;
  }

  if (event.metaKey || event.ctrlKey || event.altKey) return null;

  switch (event.key.toLowerCase()) {
    case "w":
      return "up";
    case "s":
      return "down";
    case "a":
      return "left";
    case "d":
      return "right";
    default:
      return null;
  }
}

export function shouldIgnorePageShortcut(event: KeyboardEvent): boolean {
  return (
    event.defaultPrevented ||
    event.isComposing ||
    event.metaKey ||
    event.ctrlKey ||
    event.altKey ||
    isEditableTarget(event.target) ||
    hasOpenBlockingOverlay()
  );
}
