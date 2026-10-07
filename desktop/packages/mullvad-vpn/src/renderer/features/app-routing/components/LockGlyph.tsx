import { colors } from '../../../lib/foundations';

type LockGlyphProps = { size?: number; color?: string };

// A padlock, for the apps locked to the VPN. The icon set has none.
export function LockGlyph({ size = 16, color = colors.greenText }: LockGlyphProps) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke={color}
      strokeWidth="2.2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden
      style={{ flexShrink: 0 }}>
      <rect x="5" y="11" width="14" height="10" rx="2" />
      <path d="M8 11V7a4 4 0 018 0v4" />
    </svg>
  );
}
