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
}
