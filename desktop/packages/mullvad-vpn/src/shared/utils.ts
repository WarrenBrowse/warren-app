import { checkAddress } from '@polkadot/util-crypto';

export type NonEmptyArray<T> = [T, ...T[]];

export function hasValue<T>(value: T): value is NonNullable<T> {
  return value !== undefined && value !== null;
}

export function isInRanges(value: number, ranges: [number, number][]): boolean {
  return ranges.some(([min, max]) => value >= min && value <= max);
}

export function isNumber(number: unknown): number is number {
  return !Number.isNaN(number);
}

// Substrate/SS58 network prefix for Warren wallet addresses. An
// address encoded with this prefix is 47-49 chars and starts with
// `wb` (e.g. `wb7kgy8FF4rx4tamkksPfoymeeeZVXLrnSjbBxCun3XhP9DnB`).
export const WARREN_SS58_PREFIX = 13295;

// Validates a Warren wallet pubkey: a valid SS58 address on the
// `WARREN_SS58_PREFIX` network. `checkAddress` verifies the base58
// encoding, the embedded checksum and the network prefix, throwing on
// malformed input — hence the try/catch normalising to `false`.
export function isWarrenPubKey(value: string): boolean {
  try {
    const [ok] = checkAddress(value, WARREN_SS58_PREFIX);
    return ok;
  } catch {
    return false;
  }
}

// Warren voucher format emitted by the admin panel (Crockford-32):
// alphabet `0-9` + `A-Z` minus `I L O U` (excluded for visual
// disambiguation — `I↔1`, `L↔1`, `O↔0`, `U↔V`), 16 raw chars
// (80 bits of entropy), displayed as `XXXX-XXXX-XXXX-XXXX` for
// readability. The server (`warren_api::vouchers::normalize_secret`)
// accepts both the dashed display form and the raw form — this
// regex mirrors that contract so the renderer can validate either
// shape locally before sending. Case-insensitive: the server
// uppercases on input, so a lowercased paste round-trips fine.
//
// MUST stay in sync with `warren_api::vouchers::VOUCHER_ALPHABET`.
// Char class enumerates 0-9, A-H, J-K, M-N, P-T, V-Z = 32 chars.
// Note: `L` and `O` sit between letters that are themselves valid,
// so we cannot collapse into a single A-Z range — every excluded
// letter splits the alphabet into two sub-ranges.
const WARREN_VOUCHER_ALPHABET_CHARCLASS = '[0-9A-HJKM-NP-TV-Z]';
const WARREN_VOUCHER_REGEX = new RegExp(
  // Dashed display form (4-4-4-4)…
  `^(?:${WARREN_VOUCHER_ALPHABET_CHARCLASS}{4}-){3}${WARREN_VOUCHER_ALPHABET_CHARCLASS}{4}$|` +
    // …or raw 16-char form.
    `^${WARREN_VOUCHER_ALPHABET_CHARCLASS}{16}$`,
  'i',
);

export function isWarrenVoucher(value: string): boolean {
  return WARREN_VOUCHER_REGEX.test(value);
}
