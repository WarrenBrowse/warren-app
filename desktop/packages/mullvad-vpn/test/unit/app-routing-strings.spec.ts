import { describe, expect, it } from 'vitest';

import { appRouteLineText } from '../../src/renderer/features/app-routing/strings';

describe('appRouteLineText, the line under an app with a country', () => {
  it('says a route past what the server admits waits for a free one', () => {
    expect(appRouteLineText({ kind: 'unavailable', reason: 'waiting-for-route' })).toBe(
      'Waiting for a free route',
    );
  });

  it('names a refusal of every session token as a session limit', () => {
    expect(appRouteLineText({ kind: 'unavailable', reason: 'limit-reached' })).toBe(
      'Session limit reached',
    );
  });
});
