import type { Theme } from '../../../../shared/theme';
import { SurfaceToken, surfaceTokens } from '../tokens';

function variableName(token: SurfaceToken): `--surface-${string}` {
  return `--surface-${token.replace(/[A-Z]/g, (letter) => `-${letter.toLowerCase()}`)}`;
}

const surfaceTokenNames = Object.keys(surfaceTokens.dark) as Array<SurfaceToken>;

// The values of every surface variable in one theme, for the global style to
// write under that theme's selector.
export function surfaceVariables(theme: Theme): Record<string, string> {
  return Object.fromEntries(
    surfaceTokenNames.map((token) => [variableName(token), surfaceTokens[theme][token]]),
  );
}

// Components only ever reference the variables, so switching the theme is one
// attribute on the document root and no re-render.
export const surfaces = Object.fromEntries(
  surfaceTokenNames.map((token) => [token, `var(${variableName(token)})`]),
) as Record<SurfaceToken, string>;
