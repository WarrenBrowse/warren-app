import React from 'react';

import log from '../../../../shared/logging';
import { parseThemePreference, ThemePreference } from '../../../../shared/theme';
import { useAppContext } from '../../../context';
import { useSelector } from '../../../redux/store';

export function useTheme() {
  const theme = useSelector((state) => parseThemePreference(state.settings.guiSettings.theme));
  const { setTheme: contextSetTheme } = useAppContext();

  const setTheme = React.useCallback(
    (value: ThemePreference) => {
      try {
        contextSetTheme(value);
      } catch (error) {
        const message = error instanceof Error ? error.message : '';
        log.error('Could not set the theme', message);
      }
    },
    [contextSetTheme],
  );

  return { theme, setTheme };
}
