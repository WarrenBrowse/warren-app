import { describe, expect, it } from 'vitest';

import {
  formatWarrenPubKey,
  isWarrenAddress,
  shortenWarrenPubKey,
  WARREN_SS58_PREFIX,
} from '../../src/renderer/lib/pubkey';
import { isWarrenPubKey } from '../../src/shared/utils';

// Ground-truth vectors from @polkadot/util-crypto v14, prefix 13295.
// pubkey hex 0000…0000 and abcdef…0123456789 -> SS58 addresses below.
const ZERO_ADDRESS = 'wb7kgy8FF4rx4tamkksPfoymeeeZVXLrnSjbBxCun3XhP9DnB';
const VEC2_ADDRESS = 'wbBdxLCYEJVa8tMAFTJVYn5tvHX8SUf8SZmSQRqs9ro3EXEkh';
// A valid SS58 address on a different network (Polkadot, prefix 0).
const POLKADOT_ADDRESS = '15oF4uVJwmo4TdGW7VfQxNLavjCXviqxT9S1MgbjMNHr6Sp5';

describe('Warren SS58 prefix', () => {
  it('is 13295', () => {
    expect(WARREN_SS58_PREFIX).toBe(13295);
  });
});

describe('Warren pubkey validation (isWarrenPubKey)', () => {
  it('accepts a valid Warren SS58 address (prefix 13295)', () => {
    expect(isWarrenPubKey(ZERO_ADDRESS)).to.be.true;
    expect(isWarrenPubKey(VEC2_ADDRESS)).to.be.true;
  });

  it('rejects an SS58 address from another network (Polkadot, prefix 0)', () => {
    expect(isWarrenPubKey(POLKADOT_ADDRESS)).to.be.false;
  });

  it('rejects a legacy 64-char hex pubkey', () => {
    expect(isWarrenPubKey('a'.repeat(64))).to.be.false;
    expect(isWarrenPubKey('0123456789abcdef'.repeat(4))).to.be.false;
  });

  it('rejects an address whose checksum has been tampered with', () => {
    // Flip the last char of a valid address -> checksum mismatch.
    const tampered = ZERO_ADDRESS.slice(0, -1) + (ZERO_ADDRESS.endsWith('B') ? 'C' : 'B');
    expect(isWarrenPubKey(tampered)).to.be.false;
  });

  it('rejects empty and obviously invalid strings', () => {
    expect(isWarrenPubKey('')).to.be.false;
    expect(isWarrenPubKey('deadbeef')).to.be.false;
    expect(isWarrenPubKey('not-an-address')).to.be.false;
  });

  it('rejects a Mullvad-style 16-digit account number', () => {
    expect(isWarrenPubKey('1234567890123456')).to.be.false;
  });

  it('rejects strings with embedded whitespace', () => {
    const withSpace = ZERO_ADDRESS.slice(0, 10) + ' ' + ZERO_ADDRESS.slice(10);
    expect(isWarrenPubKey(withSpace)).to.be.false;
  });
});

describe('isWarrenAddress (renderer alias)', () => {
  it('mirrors isWarrenPubKey', () => {
    expect(isWarrenAddress(ZERO_ADDRESS)).to.be.true;
    expect(isWarrenAddress(POLKADOT_ADDRESS)).to.be.false;
    expect(isWarrenAddress('')).to.be.false;
  });
});

describe('Warren pubkey formatting (shortenWarrenPubKey / formatWarrenPubKey)', () => {
  it('shortens an SS58 address to head…tail (6 + … + 6)', () => {
    expect(shortenWarrenPubKey(ZERO_ADDRESS)).toBe('wb7kgy…hP9DnB');
    expect(formatWarrenPubKey(ZERO_ADDRESS)).toBe('wb7kgy…hP9DnB');
  });

  it('shortens the second vector correctly', () => {
    expect(shortenWarrenPubKey(VEC2_ADDRESS)).toBe('wbBdxL…3EXEkh');
  });

  it('uses the U+2026 horizontal ellipsis character', () => {
    expect(formatWarrenPubKey(ZERO_ADDRESS)).toContain('…');
    expect(formatWarrenPubKey(ZERO_ADDRESS)).not.toContain('...');
  });

  it('returns strings of length <= 13 unchanged', () => {
    expect(formatWarrenPubKey('wb1234567890')).toBe('wb1234567890');
    expect(formatWarrenPubKey('1234567890123')).toBe('1234567890123');
    expect(formatWarrenPubKey('short')).toBe('short');
  });

  it('shortens strings longer than 13 chars', () => {
    expect(formatWarrenPubKey('12345678901234')).toBe('123456…901234');
  });

  it('handles empty input', () => {
    expect(formatWarrenPubKey('')).toBe('');
  });

  it('does NOT chunk the address into space-separated groups', () => {
    expect(formatWarrenPubKey(ZERO_ADDRESS)).not.toContain(' ');
  });
});
