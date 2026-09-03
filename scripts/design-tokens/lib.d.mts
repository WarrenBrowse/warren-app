// Type surface of lib.mjs for the vitest gate (the desktop tsconfig type-checks
// test/**, and a bare .mjs import would fail TS7016).

export interface Measure {
  value: number;
  unit: 'dp' | 'sp' | 'ms' | 'ratio';
}

export type ComponentTokens = Record<string, Record<string, Measure | string>>;

export interface DesignTokens {
  $comment: string;
  sources: Record<string, string>;
  colors: Record<string, string>;
  radius: Record<string, number>;
  spacing: Record<string, number>;
  typography: {
    fontFamilies: Record<string, string>;
    fontWeights: Record<string, number>;
    fontSizes: Record<string, number>;
    lineHeights: Record<string, number>;
  };
  components: ComponentTokens;
}

export const JSON_PATH: string;
export const KOTLIN_PATH: string;
export function cssColorToHex(css: string): string;
export function buildTokens(repoRoot: string): DesignTokens;
export function renderJson(tokens: DesignTokens): string;
export function renderKotlin(tokens: DesignTokens, jsonText: string): string;
export function sha256(text: string): string;
