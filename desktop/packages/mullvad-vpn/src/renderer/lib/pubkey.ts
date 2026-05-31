import { isWarrenPubKey, WARREN_SS58_PREFIX } from '../../shared/utils';

// Re-exported for callers that import the SS58 prefix from this module.
export { WARREN_SS58_PREFIX };

// Below this length there is nothing to shorten, so the address is
// returned verbatim. Mirrors Polkadot's `toShortAddress` behaviour
// (6 head + 1 ellipsis + 6 tail = 13 chars).
const SHORTEN_MIN_LENGTH = 13;
const SHORTEN_EDGE_CHARS = 6;

// U+2026 HORIZONTAL ELLIPSIS, matching Polkadot's short-address style.
const ELLIPSIS = '…';

/**
 * Returns `true` when `value` is a valid Warren SS58 address for the
 * {@link WARREN_SS58_PREFIX} network. Delegates to
 * `@polkadot/util-crypto`'s `checkAddress`, which validates the
 * base58 encoding, the embedded checksum and the network prefix.
 */
export function isWarrenAddress(value: string): boolean {
  return isWarrenPubKey(value);
}

/**
 * Shortens a Warren SS58 address for display in the
 * `toShortAddress` style: first 6 chars + `…` + last 6 chars (e.g.
 * `wb7kgy…hP9DnB`). Strings of length <= 13 are returned unchanged.
 *
 * This is display-only; callers that need the full address (e.g. for
 * clipboard copy) must use the raw value, not this output.
 */
export function shortenWarrenPubKey(pubkey: string): string {
  if (pubkey.length <= SHORTEN_MIN_LENGTH) {
    return pubkey;
  }
  return `${pubkey.substring(0, SHORTEN_EDGE_CHARS)}${ELLIPSIS}${pubkey.substring(
    pubkey.length - SHORTEN_EDGE_CHARS,
  )}`;
}

/**
 * Formats a Warren wallet pubkey (SS58 address) for display. Produces
 * the shortened `head…tail` form via {@link shortenWarrenPubKey}.
 *
 * Kept under this name because it is the widely-imported display
 * helper across the renderer. It no longer chunks a hex string — the
 * pubkey is now a single SS58 token.
 */
export function formatWarrenPubKey(pubkey: string): string {
  return shortenWarrenPubKey(pubkey);
}
