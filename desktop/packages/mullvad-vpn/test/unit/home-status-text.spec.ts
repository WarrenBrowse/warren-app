import { describe, expect, it } from 'vitest';

import { getHomeStatus } from '../../src/renderer/components/views/main/home/home-status-text';
import { TunnelState } from '../../src/shared/daemon-rpc-types';

const connected = { state: 'connected' } as TunnelState;
const connecting = { state: 'connecting' } as TunnelState;
const disconnecting = { state: 'disconnecting', details: 'nothing' } as TunnelState;
const disconnected = { state: 'disconnected', lockedDown: false } as TunnelState;
const lockedDown = { state: 'disconnected', lockedDown: true } as TunnelState;
const helsinki = { country: 'Finland', city: 'Helsinki', hostname: 'warren-3cb774609e5b6876' };
const nowhere = { country: undefined, city: undefined, hostname: undefined };

describe('getHomeStatus, the pill, title and line under it', () => {
  it('tells an exposed user they are visible', () => {
    expect(getHomeStatus(disconnected, 'exposed', nowhere, false)).toEqual({
      pill: 'Unprotected',
      title: 'You are visible',
      subtitle: 'Your connection is not encrypted',
      subtitleIsHostname: false,
    });
  });

  it('names the exit city first once protected, with the exit host under it', () => {
    expect(getHomeStatus(connected, 'protected', helsinki, false)).toEqual({
      pill: 'Protected',
      title: 'Helsinki, Finland',
      subtitle: 'warren-3cb774609e5b6876',
      subtitleIsHostname: true,
    });
  });

  it('falls back to the country alone when the exit has no city', () => {
    const status = getHomeStatus(connected, 'protected', { ...helsinki, city: undefined }, false);
    expect(status.title).toBe('Finland');
  });

  it('names a city-state once', () => {
    const singapore = { country: 'Singapore', city: 'Singapore', hostname: 'warren-1' };
    expect(getHomeStatus(connected, 'protected', singapore, false).title).toBe('Singapore');
  });

  it('says only the chosen apps are protected while include-only is on', () => {
    const status = getHomeStatus(connected, 'protected', helsinki, true);
    expect(status.subtitle).toBe('Only selected apps are protected');
    expect(status.subtitleIsHostname).toBe(false);
  });

  it('names the target while connecting, and says the tunnel is coming up', () => {
    expect(getHomeStatus(connecting, 'connecting', helsinki, false)).toEqual({
      pill: 'Connecting',
      title: 'Helsinki, Finland',
      subtitle: 'Connection in progress',
      subtitleIsHostname: false,
    });
    expect(getHomeStatus(connecting, 'connecting', nowhere, false).title).toBe(
      'Connection in progress',
    );
  });

  it('keeps the user visible while the tunnel is torn down', () => {
    const status = getHomeStatus(disconnecting, 'connecting', helsinki, false);
    expect(status.title).toBe('You are visible');
    expect(status.subtitle).toBe('Disconnecting...');
  });

  it('holds the interrupted tunnel as protected but not settled', () => {
    expect(getHomeStatus(connected, 'interrupted', helsinki, false)).toEqual({
      pill: 'Interrupted',
      title: 'Connection interrupted',
      subtitle: 'You are protected',
      subtitleIsHostname: false,
    });
  });

  it('calls a held kill switch blocked', () => {
    expect(getHomeStatus(lockedDown, 'blocked', nowhere, false)).toEqual({
      pill: 'Blocked',
      title: 'Connection blocked',
      subtitle: '',
      subtitleIsHostname: false,
    });
  });
});
