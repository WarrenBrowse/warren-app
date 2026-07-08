import { describe, expect, it } from 'vitest';

import { isUnspecifiedSocketAddress } from '../../src/shared/daemon-rpc-types';

describe('isUnspecifiedSocketAddress', () => {
  // The Warren daemon publishes the multi-hop exit endpoint as `0.0.0.0:0`
  // when the client-facing directory redacts the exit egress IP (the client
  // dials the entry relay and never learns the exit IP). Such a placeholder
  // must be recognised so the GUI does not render it as a real "Out" address.
  it('recognises the IPv4 unspecified placeholder 0.0.0.0:0', () => {
    expect(isUnspecifiedSocketAddress('0.0.0.0:0')).toBe(true);
  });

  it('recognises the IPv6 unspecified placeholder [::]:0', () => {
    expect(isUnspecifiedSocketAddress('[::]:0')).toBe(true);
  });

  it('treats a real relay endpoint as specified', () => {
    expect(isUnspecifiedSocketAddress('204.168.207.130:443')).toBe(false);
  });

  it('treats a real IPv6 endpoint as specified', () => {
    expect(isUnspecifiedSocketAddress('[2a01:4f9:c014:1098::1]:443')).toBe(false);
  });

  it('does not treat 0.0.0.0 with a real port as unspecified', () => {
    // A non-zero port means the address carries routing intent; only the
    // full 0.0.0.0:0 (host AND port unset) is the redaction placeholder.
    expect(isUnspecifiedSocketAddress('0.0.0.0:443')).toBe(false);
  });

  it('returns false for an unparsable address instead of throwing', () => {
    expect(isUnspecifiedSocketAddress('not-an-address')).toBe(false);
  });
});
