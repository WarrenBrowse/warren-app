import { homeTokens } from '../tokens';

// The home geometry as CSS custom properties (`--home-hero-fade: 40px`), the
// shape the browser extension's design sync copies, so its popup takes the
// app's measures under the app's own names.
function cssValue(key: string, value: number): string {
  if (/(^w|W)eight$/.test(key) || /(^s|S)top$/.test(key)) return String(value);
  if (/Em$/.test(key)) return `${value}em`;
  if (/Ms$/.test(key)) return `${value}ms`;
  return `${value}px`;
}

const kebab = (name: string) => name.replace(/[A-Z]/g, (letter) => `-${letter.toLowerCase()}`);

export const homeVariables: Record<string, string> = Object.fromEntries(
  Object.entries(homeTokens).flatMap(([group, entries]) =>
    Object.entries(entries).map(([key, value]) => [
      `--home-${kebab(group)}-${kebab(key)}`,
      cssValue(key, value as number),
    ]),
  ),
);
