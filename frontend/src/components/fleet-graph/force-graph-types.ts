import type { ForceGraphMethods, LinkObject, NodeObject } from 'react-force-graph-2d';
import type { GraphLink, GraphNode } from './build-force-graph-data';

export interface GraphCoords {
  x: number;
  y: number;
}

export type ForceGraphApi = ForceGraphMethods<
  NodeObject<GraphNode>,
  LinkObject<GraphNode, GraphLink>
>;

export interface ZoomTransformLike {
  k: number;
  x: number;
  y: number;
}

export type ZoomTransformConstructor = new (k: number, x: number, y: number) => ZoomTransformLike;

export type ZoomCanvas = HTMLCanvasElement & {
  __zoom?: ZoomTransformLike;
};
