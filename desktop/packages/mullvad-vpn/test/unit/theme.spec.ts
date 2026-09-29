import { describe, expect, it } from 'vitest';

import { parseThemePreference, readSystemTheme, resolveTheme } from '../../src/shared/theme';

// A stand-in for `window.matchMedia` that answers for one scheme, or for none.
function mediaAnswering(scheme: 'dark' | 'light' | undefined) {
  return (query: string) => ({
    matches: scheme !== undefined && query === `(prefers-color-scheme: ${scheme})`,
  });
}

describe('the theme preference', () => {
  it('follows the system by default', () => {
    expect(parseThemePreference(undefined)).toBe('system');
  });

  it('keeps an explicit choice', () => {
    expect(parseThemePreference('dark')).toBe('dark');
    expect(parseThemePreference('light')).toBe('light');
    expect(parseThemePreference('system')).toBe('system');
  });

  it('falls back to the system for a value this version does not know', () => {
    // A settings file written by a later version, or edited by hand, must not
    // leave the app without a theme.
    expect(parseThemePreference('sepia')).toBe('system');
    expect(parseThemePreference(1)).toBe('system');
  });
});

describe('the system theme', () => {
  it('is dark when the system says dark', () => {
    expect(readSystemTheme(mediaAnswering('dark'))).toBe('dark');
  });

  it('is light when the system says light', () => {
    expect(readSystemTheme(mediaAnswering('light'))).toBe('light');
  });

  it('is unknown when the system says neither', () => {
    expect(readSystemTheme(mediaAnswering(undefined))).toBeUndefined();
  });

  it('is unknown when there is no media query support at all', () => {
    expect(readSystemTheme(undefined)).toBeUndefined();
  });
});

describe('the theme the screens are painted in', () => {
  it('follows a light system', () => {
    expect(resolveTheme('system', 'light')).toBe('light');
  });

  it('follows a dark system', () => {
    expect(resolveTheme('system', 'dark')).toBe('dark');
  });

  it('is dark when the system theme cannot be found', () => {
    expect(resolveTheme('system', undefined)).toBe('dark');
  });

  it('keeps an explicit choice whatever the system says', () => {
    expect(resolveTheme('dark', 'light')).toBe('dark');
    expect(resolveTheme('light', 'dark')).toBe('light');
    expect(resolveTheme('light', undefined)).toBe('light');
  });
});
