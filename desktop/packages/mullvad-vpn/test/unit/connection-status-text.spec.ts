import { describe, expect, it } from 'vitest';

import { getConnectionStatusSubtitle } from '../../src/renderer/components/views/main/components/connection-panel/components/connection-status/connection-status-text';
import { TunnelState } from '../../src/shared/daemon-rpc-types';

const connected = { state: 'connected' } as TunnelState;
const connecting = { state: 'connecting' } as TunnelState;

describe('getConnectionStatusSubtitle, the line under the connection state', () => {
  it('says the device is protected once connected', () => {
    expect(getConnectionStatusSubtitle(connected, 'protected', false)).toBe('You are protected');
  });

  it('says only the chosen apps are protected while include-only is on', () => {
    expect(getConnectionStatusSubtitle(connected, 'protected', true)).toBe(
      'Only selected apps are protected',
    );
    expect(getConnectionStatusSubtitle(connected, 'interrupted', true)).toBe(
      'Only selected apps are protected',
    );
  });

  it('keeps the connecting line whatever the mode', () => {
    expect(getConnectionStatusSubtitle(connecting, 'connecting', true)).toBe(
      'Connection in progress',
    );
  });
});
