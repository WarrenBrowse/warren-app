import { describe, expect, it } from 'vitest';

import { formatExitLocation } from '../../src/shared/daemon-rpc-types';

describe('formatExitLocation', () => {
  // In multi-hop the exit egress IP is redacted, so the "Out" row falls back
  // to the exit geolocation the daemon does provide (country + city).
  it('joins country and city when both are present', () => {
    expect(formatExitLocation('Nederland', 'Amsterdam')).toBe('Nederland, Amsterdam');
  });

  it('shows the country alone when the city is missing', () => {
    expect(formatExitLocation('Nederland', undefined)).toBe('Nederland');
  });

  it('returns undefined when there is no location at all', () => {
    // Nothing to show yet (e.g. location not resolved): the caller must not
    // render an empty "Out" line.
    expect(formatExitLocation(undefined, undefined)).toBeUndefined();
  });

  it('returns undefined when only a city is known but no country', () => {
    // A city without a country is not a meaningful standalone label.
    expect(formatExitLocation(undefined, 'Amsterdam')).toBeUndefined();
  });
});
