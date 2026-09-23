import { describe, expect, it } from 'vitest';

import {
  claimCode,
  mintPurchaseClaim,
  parseClaimCode,
  pullSecretHash,
} from '../../src/main/purchase-claim';

describe('purchase claim', () => {
  it('mints a 128-bit wpid and a 256-bit pull secret, fresh every time', () => {
    const a = mintPurchaseClaim();
    const b = mintPurchaseClaim();
    expect(a.wpid).toMatch(/^[0-9a-f]{32}$/);
    expect(a.secret).toMatch(/^[0-9a-f]{64}$/);
    expect(a.wpid).not.toEqual(b.wpid);
    expect(a.secret).not.toEqual(b.secret);
  });

  it('hashes the raw secret bytes exactly as warren-api does (pinned)', () => {
    expect(pullSecretHash('11'.repeat(32))).toBe(
      '02d449a31fbb267c8f352e9968a79e3e5fc95c1bbeaa502fd6454ebde5a4bedc',
    );
  });

  it('round-trips through the daemon claim code, and refuses anything else', () => {
    const claim = mintPurchaseClaim();
    const code = claimCode(claim);
    expect(code).toMatch(/^[0-9a-f]{96}$/);
    expect(parseClaimCode(code)).toEqual(claim);
    expect(parseClaimCode(claim.wpid)).toBeUndefined();
    expect(parseClaimCode('ABCD-EFGH-JKMN-PQRS')).toBeUndefined();
    expect(parseClaimCode(`${code}0`)).toBeUndefined();
  });
});
