import { describe, expect, it } from 'vitest';

import {
  PLAINE_IMAGE,
  resolveCountryImage,
  resolveScenery,
} from '../../src/renderer/components/CountryBackdrop/scenery';
import { getConnectionPhase, getPhaseAccentColor } from '../../src/renderer/lib/connection-phase';
import { colors } from '../../src/renderer/lib/foundations';

describe('getConnectionPhase', () => {
  it('maps connected to protected', () => {
    expect(getConnectionPhase('connected')).toBe('protected');
  });

  it('maps connecting and disconnecting to the transitional phase', () => {
    expect(getConnectionPhase('connecting')).toBe('connecting');
    expect(getConnectionPhase('disconnecting')).toBe('connecting');
  });

  it('maps disconnected and error to exposed', () => {
    expect(getConnectionPhase('disconnected')).toBe('exposed');
    expect(getConnectionPhase('error')).toBe('exposed');
  });
});

describe('getPhaseAccentColor', () => {
  it('uses green when protected, orange when connecting, red when exposed', () => {
    expect(getPhaseAccentColor('protected')).toBe(colors.green);
    expect(getPhaseAccentColor('connecting')).toBe(colors.orange);
    expect(getPhaseAccentColor('exposed')).toBe(colors.red);
  });
});

describe('resolveCountryImage', () => {
  it('maps the three supported exits to their cityscape, case-insensitively', () => {
    expect(resolveCountryImage('Germany')).toMatch(/germany\.webp$/);
    expect(resolveCountryImage('netherlands')).toMatch(/netherlands\.webp$/);
    expect(resolveCountryImage('  SINGAPORE ')).toMatch(/singapore\.webp$/);
  });

  it('falls back to the plain for unknown or missing countries', () => {
    expect(resolveCountryImage('France')).toBe(PLAINE_IMAGE);
    expect(resolveCountryImage(undefined)).toBe(PLAINE_IMAGE);
    expect(resolveCountryImage('')).toBe(PLAINE_IMAGE);
  });
});

describe('resolveScenery', () => {
  it('exposed: plain, rabbit shown, no blur, regardless of country', () => {
    expect(resolveScenery('exposed', 'Germany')).toEqual({
      image: PLAINE_IMAGE,
      showBula: true,
      blurred: false,
    });
  });

  it('connecting: target cityscape, rabbit still shown, blurred', () => {
    const scenery = resolveScenery('connecting', 'Netherlands');
    expect(scenery.image).toMatch(/netherlands\.webp$/);
    expect(scenery.showBula).toBe(true);
    expect(scenery.blurred).toBe(true);
  });

  it('protected: exit cityscape, rabbit hidden, sharp', () => {
    const scenery = resolveScenery('protected', 'Singapore');
    expect(scenery.image).toMatch(/singapore\.webp$/);
    expect(scenery.showBula).toBe(false);
    expect(scenery.blurred).toBe(false);
  });

  it('protected without art still hides the rabbit on the plain', () => {
    expect(resolveScenery('protected', 'France')).toEqual({
      image: PLAINE_IMAGE,
      showBula: false,
      blurred: false,
    });
  });
});
