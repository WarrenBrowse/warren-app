// The theme the user asked for: follow the operating system, or one of the two
// palettes whatever the system says.
export type ThemePreference = 'system' | 'dark' | 'light';

// The palette the screens are actually painted in.
export type Theme = 'dark' | 'light';

export const DEFAULT_THEME_PREFERENCE: ThemePreference = 'system';

const PREFERENCES: ReadonlyArray<ThemePreference> = ['system', 'dark', 'light'];

// The stored value comes from a JSON file that a later version, or a person,
// may have written: anything this version does not know follows the system.
export function parseThemePreference(value: unknown): ThemePreference {
  return PREFERENCES.find((preference) => preference === value) ?? DEFAULT_THEME_PREFERENCE;
}

type MatchMedia = (query: string) => { matches: boolean };

// Undefined when the system states no scheme, so the caller can tell "the
// system is light" from "the system says nothing".
export function readSystemTheme(matchMedia: MatchMedia | undefined): Theme | undefined {
  if (matchMedia === undefined) {
    return undefined;
  }
  if (matchMedia('(prefers-color-scheme: dark)').matches) {
    return 'dark';
  }
  if (matchMedia('(prefers-color-scheme: light)').matches) {
    return 'light';
  }
  return undefined;
}

// Dark is the house palette: a system theme that cannot be found paints dark.
export function resolveTheme(preference: ThemePreference, systemTheme: Theme | undefined): Theme {
  if (preference === 'system') {
    return systemTheme ?? 'dark';
  }
  return preference;
}
