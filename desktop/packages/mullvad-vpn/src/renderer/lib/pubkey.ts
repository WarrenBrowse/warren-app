import { isWarrenPubKey, shortenWarrenPubKey, WARREN_SS58_PREFIX } from '../../shared/utils';

// Re-exported for callers that import these from this module.
export { shortenWarrenPubKey, WARREN_SS58_PREFIX };

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
 * Formats a Warren wallet pubkey (SS58 address) for display. Produces
 * the shortened `head…tail` form via {@link shortenWarrenPubKey}.
 *
 * Kept under this name because it is the widely-imported display
 * helper across the renderer. It no longer chunks a hex string - the
 * pubkey is now a single SS58 token.
 */
export function formatWarrenPubKey(pubkey: string): string {
  return shortenWarrenPubKey(pubkey);
}
