import { describe, expect, it } from 'vitest';

import { getNavigationBase } from '../../src/renderer/lib/functions/navigation-base';
import { LoginState } from '../../src/renderer/redux/account/reducers';
import { RoutePath } from '../../src/shared/routes';

const okLogin: LoginState = {
  type: 'ok',
  method: 'existing_account',
};

describe('getNavigationBase Warren onboarding redirect (M5.B.3)', () => {
  it('routes a logged-in user with no onboarding timestamp to the onboarding wizard', () => {
    const path = getNavigationBase(true, okLogin, undefined);
    expect(path).toBe(RoutePath.onboardingWelcome);
  });

  it('routes a logged-in user with a completed onboarding timestamp to main', () => {
    const path = getNavigationBase(true, okLogin, 1_700_000_000);
    expect(path).toBe(RoutePath.main);
  });

  it('does not redirect the login screen when the daemon is not connected, even if onboarding is unfinished', () => {
    const path = getNavigationBase(false, okLogin, undefined);
    expect(path).toBe(RoutePath.launch);
  });

  it('does not redirect to onboarding when the user is not logged in yet (logging-in state should still land on login flow)', () => {
    const path = getNavigationBase(
      true,
      { type: 'logging in', method: 'existing_account' },
      undefined,
    );
    expect(path).toBe(RoutePath.login);
  });

  it('does not redirect to onboarding when the account is expired (user must resolve billing first)', () => {
    const expiredLogin: LoginState = {
      type: 'ok',
      method: 'existing_account',
      expiredState: 'expired',
    };
    const path = getNavigationBase(true, expiredLogin, undefined);
    expect(path).toBe(RoutePath.expired);
  });
});
