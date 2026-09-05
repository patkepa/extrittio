import type { RefObject } from 'react';

declare module '@patkepa/kantzen-ui/interactions' {
  /**
   * React 19 includes the initial null value in DOM ref types. The runtime hook
   * already handles an unmounted container; this overload keeps its published
   * React 18-era declaration compatible with React 19 consumers.
   */
  export function useFormNavigation(
    containerRef: RefObject<HTMLElement | null>,
    enabled?: boolean,
  ): void;
}
