// Renderer-visible auto-renewal state (warren-core doc 65): display data only,
// never the customer handle nor the bearer token.
export interface RenewalUiState {
  /** Renewal cycle length; always 1 (decision 12.8: monthly cycles). */
  months: number;
  /** Per-month price in minor units (flat rate, decision 12.3). */
  priceCents?: number;
  currency?: string;
  cardBrand?: string;
  cardLast4?: string;
  /** Next renewal date (the account expiry), for the permanent in-app
   *  display of the pre-renewal notice. */
  renewsAtMs?: number;
  /** Last successful renewal charge (receipt display). */
  lastChargeMs?: number;
  /** The account is banned: nothing is charged until the ban ends, since a
   *  banned wallet cannot credit the time it would buy (warren-core doc 105). */
  pausedByBan?: true;
  /** Every pre-renewal notice archived on this device as a PDF, newest first
   *  (the durable medium of the notice, GEO 111/2011 art. 50). */
  notices?: RenewalNoticeSummary[];
}

export interface RenewalNoticeSummary {
  id: string;
  shownAtMs: number;
  renewsAtMs: number;
}

// Current price of one monthly cycle, and the lowest monthly price any plan
// of the same currency offers (the best-tariff information owed before each
// renewal, Lexter memo para 36(e)).
export interface RenewalPrice {
  monthlyCents: number;
  bestMonthlyCents: number;
}

// One pre-renewal notice, as displayed and archived on the device.
export interface RenewalNotice {
  renewsAtMs: number;
  shownAtMs: number;
  currency: string;
  monthlyCents?: number;
  bestMonthlyCents?: number;
  // Set when this notice announces a higher monthly price than the last one.
  previousMonthlyCents?: number;
}

export function formatMinorUnits(cents: number, currency: string, locale: string): string {
  try {
    return new Intl.NumberFormat(locale, { style: 'currency', currency }).format(cents / 100);
  } catch {
    return `${(cents / 100).toFixed(2)} ${currency}`;
  }
}
