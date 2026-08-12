import { useCallback, useRef, useState } from "react";

export interface UseRovingFocusOptions {
  itemCount: number;
  initialIndex?: number;
}

export function useRovingFocus({
  itemCount,
  initialIndex = 0,
}: UseRovingFocusOptions) {
  const itemRefs = useRef<Array<HTMLElement | null>>([]);
  const [activeIndex, setActiveIndex] = useState(initialIndex);
  const boundedActiveIndex =
    itemCount === 0 ? 0 : Math.min(Math.max(activeIndex, 0), itemCount - 1);

  const focusIndex = useCallback(
    (index: number) => {
      if (itemCount === 0) return;

      const nextIndex = Math.min(Math.max(index, 0), itemCount - 1);
      setActiveIndex(nextIndex);

      requestAnimationFrame(() => {
        const element = itemRefs.current[nextIndex];
        element?.focus({ preventScroll: true });
        element?.scrollIntoView({ block: "nearest", inline: "nearest" });
      });
    },
    [itemCount],
  );

  const getItemProps = useCallback(
    (index: number) => ({
      tabIndex: boundedActiveIndex === index ? 0 : -1,
      "data-roving-item": true,
      "data-keyboard-active": boundedActiveIndex === index ? true : undefined,
      onFocus: () => setActiveIndex(index),
    }),
    [boundedActiveIndex],
  );

  const registerItem = useCallback(
    (index: number, element: HTMLElement | null) => {
      itemRefs.current[index] = element;
    },
    [],
  );

  return {
    activeIndex: boundedActiveIndex,
    focusIndex,
    getItemProps,
    registerItem,
  };
}
