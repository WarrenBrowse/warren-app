// In-app withdrawal (rétractation) of a purchase this wallet redeemed: the
// wallet-signed declaration the daemon sends to warren-api
// (`POST /v1/account/refund/by-wpid`).

export interface WithdrawalAcknowledgement {
  // Random reference of the recorded declaration, the buyer's proof.
  reference: string;
  // When the server recorded it, RFC 3339 in UTC.
  submittedAt?: string;
  // What will be refunded, in minor units of `currency`.
  refundAmountMinor?: number;
  currency?: string;
}

export type WithdrawalFailure =
  // This wallet did not redeem that purchase (or the server does not know it).
  | 'not-proven'
  // A declaration with another refund address is already on file.
  | 'already-filed'
  // Malformed purchase id or refund address.
  | 'invalid'
  | 'no-wallet'
  // Nothing recorded, a retry can work.
  | 'unavailable';

export type WithdrawalResult =
  | { type: 'declared'; acknowledgement: WithdrawalAcknowledgement }
  | { type: 'error'; reason: WithdrawalFailure };

export interface WithdrawalRequest {
  wpid: string;
  refundAddress?: string;
}

// A purchase this app redeemed and may still withdraw from, as the account
// view shows it.
export interface WithdrawablePurchase {
  wpid: string;
  redeemedMs: number;
  // Last moment of the 14-day withdrawal period.
  deadlineMs: number;
  acknowledgement?: WithdrawalAcknowledgement;
}
