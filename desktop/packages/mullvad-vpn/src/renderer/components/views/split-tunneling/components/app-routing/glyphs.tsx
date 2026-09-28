import { colors } from '../../../../../lib/foundations';

// The route glyphs of App routing. The icon set has no shield or globe, and
// these carry their own colours: green for the VPN, ocre for outside it.

type GlyphProps = { size?: number };

export function ShieldGlyph({ size = 20 }: GlyphProps) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke={colors.greenText}
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden
      style={{ flexShrink: 0 }}>
      <path d="M12 3l8 3v6c0 4.5-3.4 8.2-8 9-4.6-.8-8-4.5-8-9V6z" />
    </svg>
  );
}

export function OutsideGlyph({ size = 20 }: GlyphProps) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke={colors.nose}
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden
      style={{ flexShrink: 0 }}>
      <path d="M14 4h6v6M20 4l-9 9M18 14v5a1 1 0 01-1 1H5a1 1 0 01-1-1V7a1 1 0 011-1h5" />
    </svg>
  );
}

export function GlobeGlyph({ size = 26 }: GlyphProps) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke={colors.whiteAlpha60}
      strokeWidth="1.8"
      strokeLinecap="round"
      aria-hidden
      style={{ flexShrink: 0 }}>
      <circle cx="12" cy="12" r="9.5" />
      <path d="M2.5 12h19M12 2.5c2.6 2.8 3.8 6 3.8 9.5s-1.2 6.7-3.8 9.5c-2.6-2.8-3.8-6-3.8-9.5s1.2-6.7 3.8-9.5z" />
    </svg>
  );
}

export function CheckBadge({ size = 24 }: GlyphProps) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" aria-hidden style={{ flexShrink: 0 }}>
      <circle cx="12" cy="12" r="11" fill={colors.green} />
      <path
        d="M7 12.5l3.2 3.2L17 9"
        fill="none"
        stroke={colors.white}
        strokeWidth="2.4"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}

export function InfoGlyph({ size = 18 }: GlyphProps) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" aria-hidden style={{ flexShrink: 0 }}>
      <circle cx="12" cy="12" r="11" fill={colors.whiteOnDarkBlue20} />
      <path
        d="M12 11v6M12 7.5v.01"
        stroke={colors.darkBlue}
        strokeWidth="2.4"
        strokeLinecap="round"
      />
    </svg>
  );
}

export function WarningGlyph({ size = 18 }: GlyphProps) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden
      style={{ flexShrink: 0 }}>
      <path d="M10.3 3.9L1.8 18a2 2 0 001.7 3h17a2 2 0 001.7-3L13.7 3.9a2 2 0 00-3.4 0z" />
      <path d="M12 9v4M12 17h.01" />
    </svg>
  );
}
