// Small inline glyphs drawn in `currentColor`, so they take the colour of the
// text they sit in. The icon set has no person or arrow.

export function PersonGlyph({ size = 10 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 10 10" aria-hidden fill="currentColor">
      <circle cx="5" cy="2.8" r="2.2" />
      <path d="M0.8 10c0-2.6 1.9-4.3 4.2-4.3s4.2 1.7 4.2 4.3z" />
    </svg>
  );
}

export function ArrowGlyph({ direction, size = 10 }: { direction: 'down' | 'up'; size?: number }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 10 10"
      aria-hidden
      fill="none"
      stroke="currentColor"
      strokeWidth="1.6"
      strokeLinecap="round"
      strokeLinejoin="round"
      style={direction === 'up' ? { transform: 'rotate(180deg)' } : undefined}>
      <path d="M5 1.2v7.2M1.8 5.4 5 8.6l3.2-3.2" />
    </svg>
  );
}

export function InfoGlyph({ size = 12 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 12 12" aria-hidden fill="none">
      <circle cx="6" cy="6" r="5.25" stroke="currentColor" strokeWidth="1.2" />
      <path d="M6 5.3v3.2" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" />
      <circle cx="6" cy="3.5" r="0.8" fill="currentColor" />
    </svg>
  );
}
