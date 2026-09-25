import { describe, expect, it } from 'vitest';

import { checkLanNetwork } from '../../src/renderer/features/lan-sharing/utils';

describe('checkLanNetwork', () => {
  it('accepts IPv4 and IPv6 networks in CIDR notation', () => {
    expect(checkLanNetwork('192.168.1.0/24')).toBeUndefined();
    expect(checkLanNetwork('400::/7')).toBeUndefined();
  });

  it('ignores surrounding whitespace', () => {
    expect(checkLanNetwork(' 10.0.0.0/8 ')).toBeUndefined();
  });

  it('rejects an address without a prefix length', () => {
    expect(checkLanNetwork('192.168.1.10')).toBe('invalid');
  });

  it('rejects something that is not a network', () => {
    expect(checkLanNetwork('my-server/24')).toBe('invalid');
    expect(checkLanNetwork('10.0.0.0/33')).toBe('invalid');
  });

  // Same limits as the daemon: the widest built-in ranges are 10.0.0.0/8 and fc00::/7.
  it('rejects an IPv4 network wider than a /8', () => {
    expect(checkLanNetwork('0.0.0.0/0')).toBe('too-broad');
    expect(checkLanNetwork('10.0.0.0/7')).toBe('too-broad');
  });

  it('rejects an IPv6 network wider than a /7', () => {
    expect(checkLanNetwork('::/0')).toBe('too-broad');
    expect(checkLanNetwork('8000::/6')).toBe('too-broad');
  });
});
