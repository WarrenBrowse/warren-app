import { useEffect, useLayoutEffect, useState } from 'react';

import {
  parseThemePreference,
  readSystemTheme,
  resolveTheme,
  Theme,
} from '../../../../shared/theme';
import { useSelector } from '../../../redux/store';

const SCHEMES: ReadonlyArray<Theme> = ['dark', 'light'];

function currentSystemTheme(): Theme | undefined {
  return readSystemTheme(window.matchMedia ? (query) => window.matchMedia(query) : undefined);
}

// Chromium answers prefers-color-scheme from the operating system and fires a
// change when the user switches it, so a window left open follows along.
function useSystemTheme(): Theme | undefined {
  const [systemTheme, setSystemTheme] = useState(currentSystemTheme);

  useEffect(() => {
    if (!window.matchMedia) {
      return undefined;
    }
    const update = () => setSystemTheme(currentSystemTheme());
    const queries = SCHEMES.map((scheme) => window.matchMedia(`(prefers-color-scheme: ${scheme})`));
    queries.forEach((query) => query.addEventListener('change', update));
    return () => queries.forEach((query) => query.removeEventListener('change', update));
  }, []);

  return systemTheme;
}

// Writes the resolved theme on the document root, where the global style keys
// the surface variables on it. A layout effect, so a theme change never paints
// one frame in the old palette.
export function useDocumentTheme(): Theme {
  const preference = useSelector((state) => parseThemePreference(state.settings.guiSettings.theme));
  const theme = resolveTheme(preference, useSystemTheme());

  useLayoutEffect(() => {
    document.documentElement.dataset.theme = theme;
  }, [theme]);

  return theme;
}
