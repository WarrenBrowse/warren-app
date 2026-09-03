// Type surface of lib.mjs for the vitest gate.

export const FLAGS_DIR: string;
export const DRAWABLE_DIR: string;
export const KOTLIN_PATH: string;
export function flagCodes(repoRoot: string): string[];
export function expandHex(fill: string): string;
export function circlePath(cx: number, cy: number, r: number): string;
export function ellipsePath(cx: number, cy: number, rx: number, ry: number): string;
export function rectPath(
  x: number,
  y: number,
  w: number,
  h: number,
  rx?: number,
  ry?: number,
): string;
export function svgToPaths(svg: string): Array<{ fill: string; d: string }>;
export function svgToVectorDrawable(svg: string, code: string): string;
export function drawableName(code: string): string;
export function renderFlagAssets(codes: string[]): string;
export function buildFlagOutputs(repoRoot: string): Record<string, string>;
