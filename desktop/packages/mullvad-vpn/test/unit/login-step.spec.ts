import { describe, expect, it } from 'vitest';

import { loginViewStep } from '../../src/renderer/lib/functions/login-step';

// The login view stays mounted for a moment after the account state
// settles (navigation transition, scheduled reset). During that window
// it must NEVER fall back to the pick/welcome step: that is the
// "welcome screen flashes before the congrats screen" bug.
describe('loginViewStep', () => {
  it('shows the transition step, never pick, once logged in', () => {
    expect(loginViewStep('ok', 'pick')).toBe('loggedIn');
  });

  it('shows the transition step once logged in even from the restore step', () => {
    expect(loginViewStep('ok', 'restore')).toBe('loggedIn');
  });

  it('holds the mandatory backup step while the phrase is unconfirmed', () => {
    expect(loginViewStep('backup-pending', 'pick')).toBe('backup');
  });

  it('backup wins over a previously selected restore mode', () => {
    expect(loginViewStep('backup-pending', 'restore')).toBe('backup');
  });

  it('shows the restore step when the user picked restore', () => {
    expect(loginViewStep('none', 'restore')).toBe('restore');
  });

  it('shows the pick step for a logged-out account', () => {
    expect(loginViewStep('none', 'pick')).toBe('pick');
  });

  it('keeps the pick step while the daemon mints a new identity', () => {
    expect(loginViewStep('logging in', 'pick')).toBe('pick');
  });

  it('keeps the pick step after a failed account creation', () => {
    expect(loginViewStep('failed', 'pick')).toBe('pick');
  });
});
