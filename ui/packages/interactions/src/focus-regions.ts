export type FocusRegionDirection = "left" | "right";

const FOCUS_REGION_SELECTOR = "[data-focus-region]";
const INITIAL_FOCUS_SELECTOR = '[data-focus-region-initial="true"]';
const KEYBOARD_REGION_ATTRIBUTE = "data-focus-region-keyboard";
const FOCUSABLE_SELECTOR = [
  "button:not([disabled])",
  "a[href]",
  "input:not([disabled])",
  "select:not([disabled])",
  "textarea:not([disabled])",
  '[tabindex]:not([tabindex="-1"])',
].join(",");

const REGION_ORDER = ["sidebar", "main", "aside"];

function isVisibleElement(element: HTMLElement): boolean {
  if (element.getAttribute("aria-hidden") === "true") return false;
  const style = window.getComputedStyle(element);
  return (
    style.display !== "none" &&
    style.visibility !== "hidden" &&
    style.opacity !== "0"
  );
}

function getRegionElements(): HTMLElement[] {
  return REGION_ORDER.flatMap((region) =>
    Array.from(
      document.querySelectorAll<HTMLElement>(`[data-focus-region="${region}"]`),
    ).filter(isVisibleElement),
  );
}

function getCurrentRegionIndex(regions: HTMLElement[]): number {
  const activeElement = document.activeElement;
  if (!(activeElement instanceof HTMLElement)) return -1;

  const currentRegion = activeElement.closest<HTMLElement>(
    FOCUS_REGION_SELECTOR,
  );
  if (!currentRegion) return -1;

  return regions.indexOf(currentRegion);
}

function getFocusTarget(region: HTMLElement): HTMLElement {
  const isInNestedRegion = (element: HTMLElement) => {
    const closestRegion = element.closest<HTMLElement>(FOCUS_REGION_SELECTOR);
    return closestRegion !== null && closestRegion !== region;
  };

  const initialTarget = Array.from(
    region.querySelectorAll<HTMLElement>(INITIAL_FOCUS_SELECTOR),
  ).find((element) => isVisibleElement(element) && !isInNestedRegion(element));
  if (initialTarget && isVisibleElement(initialTarget)) return initialTarget;

  const focusableTarget = Array.from(
    region.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR),
  ).find((element) => isVisibleElement(element) && !isInNestedRegion(element));
  return focusableTarget ?? region;
}

export function clearKeyboardFocusRegions() {
  document
    .querySelectorAll<HTMLElement>(`[${KEYBOARD_REGION_ATTRIBUTE}="true"]`)
    .forEach((element) => element.removeAttribute(KEYBOARD_REGION_ATTRIBUTE));
}

export function moveFocusRegion(direction: FocusRegionDirection) {
  const regions = getRegionElements();
  if (regions.length === 0) return;

  const currentIndex = getCurrentRegionIndex(regions);
  const fallbackIndex = direction === "right" ? 0 : regions.length - 1;
  const nextIndex =
    currentIndex === -1
      ? fallbackIndex
      : Math.min(
          Math.max(currentIndex + (direction === "right" ? 1 : -1), 0),
          regions.length - 1,
        );

  const targetRegion = regions[nextIndex];
  if (!targetRegion) return;

  clearKeyboardFocusRegions();
  targetRegion.setAttribute(KEYBOARD_REGION_ATTRIBUTE, "true");
  getFocusTarget(targetRegion).focus({ preventScroll: true });
}
