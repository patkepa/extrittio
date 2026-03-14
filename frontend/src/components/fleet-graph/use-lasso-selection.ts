import { useCallback, useEffect, useRef, useState } from 'react';
import type { GraphData, GraphNode } from './build-force-graph-data';
import { useSelectionStore } from '../../stores/selection-store';
import { SELECTION_COLOR } from './constants';

const FLEET_RADIUS = 14;
const DEVICE_RADIUS = 11;

export function useLassoSelection(
  graphRef: React.MutableRefObject<any>,
  canvasWrapperRef: React.RefObject<HTMLDivElement | null>,
  graphData: GraphData,
) {
  const lassoRef = useRef<{ x1: number; y1: number; x2: number; y2: number } | null>(null);
  const isLassoingRef = useRef(false);
  const [shiftHeld, setShiftHeld] = useState(false);
  const shiftHeldRef = useRef(false);

  const selectedDeviceIds = useSelectionStore((s) => s.selectedDeviceIds);
  const addToSelection = useSelectionStore((s) => s.addToSelection);
  const toggleDevice = useSelectionStore((s) => s.toggleDevice);
  const clearSelection = useSelectionStore((s) => s.clearSelection);

  // Keep a ref so the lasso mousedown handler can read selection without re-registering
  const selectionRef = useRef(selectedDeviceIds);
  selectionRef.current = selectedDeviceIds;

  // Shift key tracking for lasso mode
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Shift') {
        shiftHeldRef.current = true;
        setShiftHeld(true);
      }
    };
    const onKeyUp = (e: KeyboardEvent) => {
      if (e.key === 'Shift') {
        shiftHeldRef.current = false;
        setShiftHeld(false);
      }
    };
    window.addEventListener('keydown', onKeyDown);
    window.addEventListener('keyup', onKeyUp);
    return () => {
      window.removeEventListener('keydown', onKeyDown);
      window.removeEventListener('keyup', onKeyUp);
    };
  }, []);

  // Lasso mouse event handlers
  useEffect(() => {
    const fg = graphRef.current;
    const wrapper = canvasWrapperRef.current;
    if (!fg || !wrapper) return;
    const canvasEl = wrapper.querySelector('canvas');
    if (!canvasEl) return;

    const onMouseDown = (e: MouseEvent) => {
      if (e.button !== 0) return;

      // Immediately deselect when clicking empty background (no shift, no node)
      if (!e.shiftKey && selectionRef.current.size > 0) {
        const coords = fg.screen2GraphCoords(e.offsetX, e.offsetY);
        const hitNode = graphData.nodes.some((node: GraphNode) => {
          if (node.x == null || node.y == null) return false;
          const r = node.type === 'fleet' ? FLEET_RADIUS : DEVICE_RADIUS;
          const dx = node.x - coords.x;
          const dy = node.y - coords.y;
          return dx * dx + dy * dy <= r * r;
        });
        if (!hitNode) {
          clearSelection();
        }
      }

      if (!e.shiftKey) return;
      const coords = fg.screen2GraphCoords(e.offsetX, e.offsetY);
      lassoRef.current = { x1: coords.x, y1: coords.y, x2: coords.x, y2: coords.y };
      isLassoingRef.current = true;
    };

    const onMouseMove = (e: MouseEvent) => {
      if (!isLassoingRef.current || !lassoRef.current) return;
      const coords = fg.screen2GraphCoords(e.offsetX, e.offsetY);
      lassoRef.current.x2 = coords.x;
      lassoRef.current.y2 = coords.y;
    };

    const onMouseUp = () => {
      if (!isLassoingRef.current || !lassoRef.current) return;
      isLassoingRef.current = false;

      const { x1, y1, x2, y2 } = lassoRef.current;
      const minX = Math.min(x1, x2);
      const maxX = Math.max(x1, x2);
      const minY = Math.min(y1, y2);
      const maxY = Math.max(y1, y2);

      if (maxX - minX > 2 && maxY - minY > 2) {
        const hitIds: string[] = [];
        for (const node of graphData.nodes) {
          if (
            node.type === 'device' &&
            node.x != null &&
            node.y != null &&
            node.x >= minX &&
            node.x <= maxX &&
            node.y >= minY &&
            node.y <= maxY
          ) {
            hitIds.push(node.id);
          }
        }
        if (hitIds.length > 0) {
          addToSelection(hitIds);
        }
      }

      lassoRef.current = null;
    };

    canvasEl.addEventListener('mousedown', onMouseDown);
    canvasEl.addEventListener('mousemove', onMouseMove);
    // Listen on window so releasing the mouse outside the canvas still
    // completes the lasso (prevents stuck lasso state).
    window.addEventListener('mouseup', onMouseUp);
    return () => {
      canvasEl.removeEventListener('mousedown', onMouseDown);
      canvasEl.removeEventListener('mousemove', onMouseMove);
      window.removeEventListener('mouseup', onMouseUp);
    };
  }, [graphRef, canvasWrapperRef, graphData.nodes, addToSelection, clearSelection]);

  // Disable pan when Shift is held (lasso mode)
  const enablePanInteraction = useCallback((ev: MouseEvent) => !ev.shiftKey, []);

  // Draw lasso selection rectangle overlay
  const paintLasso = useCallback((ctx: CanvasRenderingContext2D, globalScale: number) => {
    if (!lassoRef.current) return;
    const { x1, y1, x2, y2 } = lassoRef.current;
    const x = Math.min(x1, x2);
    const y = Math.min(y1, y2);
    const w = Math.abs(x2 - x1);
    const h = Math.abs(y2 - y1);

    ctx.save();
    ctx.fillStyle = 'rgba(45, 114, 210, 0.15)';
    ctx.fillRect(x, y, w, h);
    ctx.strokeStyle = SELECTION_COLOR;
    ctx.lineWidth = 1 / globalScale;
    ctx.setLineDash([4 / globalScale, 4 / globalScale]);
    ctx.strokeRect(x, y, w, h);
    ctx.setLineDash([]);
    ctx.restore();
  }, []);

  return {
    shiftHeld,
    selectedDeviceIds,
    toggleDevice,
    clearSelection,
    enablePanInteraction,
    paintLasso,
  };
}
