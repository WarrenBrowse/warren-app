import { describe, expect, it } from 'vitest';

import { logoutErasesIdentity } from '../../src/shared/daemon-rpc-types';

describe('logoutErasesIdentity', () => {
  it('erases the wallet for the backup-confirmed log out button', () => {
    expect(logoutErasesIdentity('gui-logout-button')).toBe(true);
  });

  it('keeps the wallet when the server revoked the device', () => {
    expect(logoutErasesIdentity('gui-device-revoked')).toBe(false);
  });

  it('keeps the wallet when a new account is discarded', () => {
    expect(logoutErasesIdentity('gui-discard-new-account')).toBe(false);
  });
});
