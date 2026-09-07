import assert from 'node:assert/strict';
import test from 'node:test';
import {
  paintNodeHitArea,
  nodeRadius,
  externalNodeSide,
} from '../src/components/fleet-graph/node-geometry.ts';
import type { GraphNode } from '../src/components/fleet-graph/build-force-graph-data.ts';

test('external-node hit area covers the entire hover-expanded square', () => {
  const rectangles: number[][] = [];
  const ctx = {
    fillStyle: '',
    fillRect: (...rectangle: number[]) => rectangles.push(rectangle),
  } as unknown as CanvasRenderingContext2D;
  const node = { type: 'external', x: 20, y: 30 } as GraphNode;
  paintNodeHitArea(node, '#123456', ctx);
  const [x, y, width, height] = rectangles[0];
  const renderedSide = externalNodeSide(nodeRadius(node, true));
  assert.equal(width, renderedSide);
  assert.equal(height, renderedSide);
  assert.equal(x + width / 2, node.x);
  assert.equal(y + height / 2, node.y);
  assert.equal(ctx.fillStyle, '#123456');
});
test('hit areas ignore unpositioned nodes and measure fleet labels using the render font', () => {
  const rectangles: number[][] = [];
  const ctx = {
    font: '',
    fillStyle: '',
    fillRect: (...rectangle: number[]) => rectangles.push(rectangle),
    measureText: () => ({ width: 100 }),
  } as unknown as CanvasRenderingContext2D;
  paintNodeHitArea({ type: 'fleet', name: 'Fleet' } as GraphNode, 'red', ctx);
  assert.equal(rectangles.length, 0);
  paintNodeHitArea({ type: 'fleet', name: 'Fleet', x: 0, y: 0 } as GraphNode, 'red', ctx);
  assert.match(ctx.font, /10px/);
  assert.ok(rectangles[0][2] > 100);
  assert.equal(rectangles[0][0] + rectangles[0][2] / 2, 0);
});
