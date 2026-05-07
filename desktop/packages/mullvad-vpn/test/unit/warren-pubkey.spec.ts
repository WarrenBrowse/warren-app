import { describe, expect, it } from 'vitest';

import { formatWarrenPubKey } from '../../src/renderer/lib/pubkey';
import { isWarrenPubKey } from '../../src/shared/utils';

describe('Warren pubkey validation (isWarrenPubKey)', () => {
  it('accepts a 64-char lowercase hex string', () => {
    const pk = 'a'.repeat(64);
    expect(isWarrenPubKey(pk)).to.be.true;
  });

  it('accepts a 64-char uppercase hex string', () => {
    const pk = 'A'.repeat(64);
    expect(isWarrenPubKey(pk)).to.be.true;
  });

  it('accepts a 64-char mixed-case hex string', () => {
    const pk = '0123456789abcdef0123456789ABCDEF0123456789abcdef0123456789ABCDEF';
    expect(isWarrenPubKey(pk)).to.be.true;
  });

  it('rejects strings shorter than 64 chars', () => {
    expect(isWarrenPubKey('a'.repeat(63))).to.be.false;
    expect(isWarrenPubKey('')).to.be.false;
    expect(isWarrenPubKey('deadbeef')).to.be.false;
  });

  it('rejects strings longer than 64 chars', () => {
    expect(isWarrenPubKey('a'.repeat(65))).to.be.false;
    expect(isWarrenPubKey('a'.repeat(128))).to.be.false;
  });

  it('rejects strings with non-hex characters', () => {
    expect(isWarrenPubKey('g'.repeat(64))).to.be.false;
    expect(isWarrenPubKey('z'.repeat(64))).to.be.false;
    expect(isWarrenPubKey('!'.repeat(64))).to.be.false;
    // 63 hex chars + 1 non-hex
    expect(isWarrenPubKey('a'.repeat(63) + 'g')).to.be.false;
  });

  it('rejects strings with whitespace', () => {
    // 64 hex chars but with embedded space → reject (caller should sanitize first)
    const withSpace = 'a'.repeat(31) + ' ' + 'a'.repeat(32);
    expect(isWarrenPubKey(withSpace)).to.be.false;
  });

  it('rejects a Mullvad-style 16-digit account number', () => {
    expect(isWarrenPubKey('1234567890123456')).to.be.false;
  });
});

describe('Warren pubkey formatting (formatWarrenPubKey)', () => {
  it('groups 64 hex chars into 8 blocks of 8 separated by spaces', () => {
    const pk = '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef';
    expect(formatWarrenPubKey(pk)).toBe(
      '01234567 89abcdef 01234567 89abcdef 01234567 89abcdef 01234567 89abcdef',
    );
  });

  it('strips embedded whitespace before grouping', () => {
    const pk = '0123 4567 89ab cdef ' + '0123456789abcdef'.repeat(3);
    expect(formatWarrenPubKey(pk)).toBe(
      '01234567 89abcdef 01234567 89abcdef 01234567 89abcdef 01234567 89abcdef',
    );
  });

  it('truncates input longer than 64 chars to the first 64', () => {
    const pk = 'a'.repeat(64) + 'EXTRA';
    expect(formatWarrenPubKey(pk)).toBe(
      'aaaaaaaa aaaaaaaa aaaaaaaa aaaaaaaa aaaaaaaa aaaaaaaa aaaaaaaa aaaaaaaa',
    );
  });

  it('handles partial input gracefully (groups what is there)', () => {
    expect(formatWarrenPubKey('abcd')).toBe('abcd');
    expect(formatWarrenPubKey('abcdef01')).toBe('abcdef01');
    expect(formatWarrenPubKey('abcdef0123456789')).toBe('abcdef01 23456789');
  });

  it('handles empty input', () => {
    expect(formatWarrenPubKey('')).toBe('');
  });
});
