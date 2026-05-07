const WARREN_PUBKEY_HEX_LEN = 64;
const WARREN_PUBKEY_GROUP_SIZE = 8;

export function formatWarrenPubKey(pubkey: string): string {
  const sanitized = pubkey.replace(/\s+/g, '').substring(0, WARREN_PUBKEY_HEX_LEN);
  const groups = sanitized.match(new RegExp(`.{1,${WARREN_PUBKEY_GROUP_SIZE}}`, 'g'));
  return groups ? groups.join(' ') : '';
}
