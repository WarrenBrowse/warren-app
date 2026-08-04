// Render a long hex fingerprint as "aabbccdd...11223344" so the UI
// can show the head + tail without breaking the layout. Returns the
// input unchanged when it is short enough to fit (<= 2 * `chars`).
export function truncatePubkeyHex(hex: string, chars = 8): string {
  if (hex.length <= 2 * chars) {
    return hex;
  }
  return `${hex.slice(0, chars)}...${hex.slice(-chars)}`;
}
