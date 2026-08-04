import { describe, expect, it } from 'vitest';

import {
  PLAINE_IMAGE,
  resolveCountryImage,
  resolveScenery,
} from '../../src/renderer/components/CountryBackdrop/scenery';
import {
  getConnectionPhase,
  getPhaseAccentColorName,
  getPhaseTitleColorName,
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

  it('connected while the host is offline degrades to interrupted, never green', () => {
    expect(getConnectionPhase(connected, true)).toBe('interrupted');
    expect(getConnectionPhase(connected, false)).toBe('protected');
  });

  it('host offline only degrades the connected state; other states keep their phase', () => {
    // Their own presentation (banner, blocked scene) already tells the
    // truth; only the green "protected" reading is a lie while offline.
    expect(getConnectionPhase(disconnectedOpen, true)).toBe('exposed');
    expect(getConnectionPhase(errorNonBlocking, true)).toBe('blocked');
    expect(getConnectionPhase(connecting, true)).toBe('connecting');
  });

  it('connected with the exit not forwarding degrades to interrupted, never green', () => {
    // Doc 62 item 5: a drained/half-swapped exit keeps the QUIC session
    // alive, so the tunnel state stays Connected while nothing flows.
    expect(getConnectionPhase(connected, false, true)).toBe('interrupted');
    expect(getConnectionPhase(connected, false, false)).toBe('protected');
  });

  it('exit egress dead only degrades the connected state', () => {
    expect(getConnectionPhase(disconnectedOpen, false, true)).toBe('exposed');
    expect(getConnectionPhase(errorNonBlocking, false, true)).toBe('blocked');
    expect(getConnectionPhase(connecting, false, true)).toBe('connecting');
  });
});

describe('getPhaseAccentColorName', () => {
  it('maps each phase to its accent token, blocked staying neutral', () => {
    expect(getPhaseAccentColorName('protected')).toBe('green');
    expect(getPhaseAccentColorName('connecting')).toBe('orange');
    expect(getPhaseAccentColorName('interrupted')).toBe('orange');
    expect(getPhaseAccentColorName('exposed')).toBe('red');
    expect(getPhaseAccentColorName('blocked')).toBe('white');
  });
});

describe('getPhaseTitleColorName', () => {
  it('titles use the lifted tint of their phase accent, never the fill tone', () => {
    // The saturated accents are sized for icons and fills. At title size on the
    // card's neutral surface they land under the 4.5:1 floor, so the text takes
    // a lifted tint while the rail and the badge keep the saturated original.
    expect(getPhaseTitleColorName('protected')).toBe('greenText');
    expect(getPhaseTitleColorName('connecting')).toBe('orangeText');
    expect(getPhaseTitleColorName('interrupted')).toBe('orangeText');
    expect(getPhaseTitleColorName('exposed')).toBe('redText');
  });

  it('blocked keeps the neutral title, it has no phase hue of its own', () => {
    expect(getPhaseTitleColorName('blocked')).toBe('white');
  });

  it('every phase gets a title tint distinct from its fill, except blocked', () => {
    const phases = ['protected', 'connecting', 'interrupted', 'exposed'] as const;
    for (const phase of phases) {
      expect(getPhaseTitleColorName(phase)).not.toBe(getPhaseAccentColorName(phase));
    }
    expect(getPhaseTitleColorName('blocked')).toBe(getPhaseAccentColorName('blocked'));
  });
});

describe('resolveScenery interrupted', () => {
  it('keeps the exit landscape but blurred, rabbit still in the burrow', () => {
    const scenery = resolveScenery('interrupted', 'netherlands');
    expect(scenery.image).toMatch(/netherlands\.webp$/);
    expect(scenery.blurred).toBe(true);
    expect(scenery.showBula).toBe(false);
  });
});

describe('resolveCountryImage', () => {
  it('maps the supported exits to their cityscape, case-insensitively', () => {
    expect(resolveCountryImage('Germany')).toMatch(/germany\.webp$/);
    expect(resolveCountryImage('netherlands')).toMatch(/netherlands\.webp$/);
    expect(resolveCountryImage('  SINGAPORE ')).toMatch(/singapore\.webp$/);
    expect(resolveCountryImage('Finland')).toMatch(/finland\.webp$/);
  });

  it('falls back to the plain for unknown or missing countries', () => {
    expect(resolveCountryImage('France')).toBe(PLAINE_IMAGE);
    expect(resolveCountryImage(undefined)).toBe(PLAINE_IMAGE);
    expect(resolveCountryImage('')).toBe(PLAINE_IMAGE);
  });
});

describe('resolveScenery', () => {
  it('exposed: the watched plain, rabbit shown, no blur', () => {
    // Unprotected is the state the art has to make felt: the open plain with
    // the cameras trained on it, sharp, and the rabbit out there blindfolded.
    // The exit country is ignored on purpose, there is no tunnel to it.
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

  it('blocked: the watched plain blurred, rabbit hidden (safe)', () => {
    // Kill switch: the rabbit is tucked in, so the world outside is only seen
    // through the blur, never sharp like the exposed state.
    expect(resolveScenery('blocked', 'Germany')).toEqual({
      image: PLAINE_IMAGE,
      showBula: false,
      blurred: true,
    });
  });

  it('the country phases fall back to the plain for an exit with no art', () => {
    expect(resolveScenery('connecting', 'France').image).toBe(PLAINE_IMAGE);
    expect(resolveScenery('protected', undefined).image).toBe(PLAINE_IMAGE);
  });
});
