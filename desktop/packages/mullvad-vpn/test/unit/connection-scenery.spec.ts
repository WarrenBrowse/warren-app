import { describe, expect, it } from 'vitest';

import {
  PLAINE_IMAGE,
  resolveCountryImage,
  resolveScenery,
} from '../../src/renderer/components/CountryBackdrop/scenery';
import {
  getConnectionPhase,
  getPhaseAccentColorName,
} from '../../src/renderer/lib/connection-phase';
import { TunnelState } from '../../src/shared/daemon-rpc-types';

const connected = { state: 'connected', details: {} } as unknown as TunnelState;
const connecting = { state: 'connecting' } as TunnelState;
const disconnecting = { state: 'disconnecting', details: 'nothing' } as TunnelState;
const disconnectedOpen = { state: 'disconnected', lockedDown: false } as TunnelState;
const disconnectedLocked = { state: 'disconnected', lockedDown: true } as TunnelState;
const errorBlocking = {
  state: 'error',
  details: { blockingError: {} },
} as unknown as TunnelState;
const errorNonBlocking = { state: 'error', details: {} } as unknown as TunnelState;

describe('getConnectionPhase', () => {
  it('connected is protected, (dis)connecting is transitional', () => {
    expect(getConnectionPhase(connected)).toBe('protected');
    expect(getConnectionPhase(connecting)).toBe('connecting');
    expect(getConnectionPhase(disconnecting)).toBe('connecting');
  });

  it('disconnected is exposed, but locked-down is blocked (kill switch)', () => {
    expect(getConnectionPhase(disconnectedOpen)).toBe('exposed');
    expect(getConnectionPhase(disconnectedLocked)).toBe('blocked');
  });

  it('a blocking error is exposed (leaking); a held block is blocked, never protected', () => {
    expect(getConnectionPhase(errorBlocking)).toBe('exposed');
    expect(getConnectionPhase(errorNonBlocking)).toBe('blocked');
  });
});

describe('getPhaseAccentColorName', () => {
  it('maps each phase to its accent token, blocked staying neutral', () => {
    expect(getPhaseAccentColorName('protected')).toBe('green');
    expect(getPhaseAccentColorName('connecting')).toBe('orange');
    expect(getPhaseAccentColorName('exposed')).toBe('red');
    expect(getPhaseAccentColorName('blocked')).toBe('white');
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
  it('exposed: plain, rabbit shown, no blur', () => {
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

  it('blocked: neutral plain, rabbit hidden (safe), sharp', () => {
    expect(resolveScenery('blocked', 'Germany')).toEqual({
      image: PLAINE_IMAGE,
      showBula: false,
      blurred: false,
    });
  });
});
