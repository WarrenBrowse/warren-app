import { createHash, randomBytes } from 'crypto';

// What the app keeps to collect a purchase it opened in the browser
// (warren-core doc 35 section 14). The wpid names the purchase and travels
// in the checkout URL, so it proves nothing on its own. The pull secret is
// what warren-api asks for before it hands out the voucher or the renewal
// handoff: only its SHA-256 goes to the checkout (the `ph` query
// parameter), and the secret itself travels only in the pull's body.
export interface PurchaseClaim {
  wpid: string;
  secret: string;
}

const CLAIM_CODE_RE = /^([0-9a-f]{32})([0-9a-f]{64})$/;

export function mintPurchaseClaim(): PurchaseClaim {
  return {
    wpid: randomBytes(16).toString('hex'),
    secret: randomBytes(32).toString('hex'),
  };
}

// SHA-256 of the raw secret bytes: the digest warren-api stores.
export function pullSecretHash(secretHex: string): string {
  return createHash('sha256').update(Buffer.from(secretHex, 'hex')).digest('hex');
}

// The form the daemon's submitVoucher takes for a purchase: wpid then
// secret, 96 hex characters. A voucher code (16 Crockford-32 characters)
// can never take that shape, so the daemon dispatches on it.
export function claimCode(claim: PurchaseClaim): string {
  return `${claim.wpid}${claim.secret}`;
}

export function parseClaimCode(code: string): PurchaseClaim | undefined {
  const match = CLAIM_CODE_RE.exec(code);
  return match ? { wpid: match[1], secret: match[2] } : undefined;
}
