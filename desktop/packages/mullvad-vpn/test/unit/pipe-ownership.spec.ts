import { describe, expect, it } from 'vitest';

import { assertPipeAdminOwned } from '../../src/main/pipe-ownership';

const PIPE = '//./pipe/Warren VPN';

describe('assertPipeAdminOwned', () => {
  it('accepts a pipe an administrator owns', () => {
    expect(() => assertPipeAdminOwned(() => true, PIPE)).not.toThrow();
  });

  it('refuses a pipe an administrator does not own', () => {
    expect(() => assertPipeAdminOwned(() => false, PIPE)).toThrow(/not owned by an admin/);
  });

  it('refuses a pipe whose owner cannot be read', () => {
    const unreadable = () => {
      throw new Error('access denied');
    };

    expect(() => assertPipeAdminOwned(unreadable, PIPE)).toThrow(/access denied/);
  });
});
