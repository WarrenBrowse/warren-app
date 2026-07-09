import { colors } from '../../foundations';

export interface LogoProps {
  variant?: 'icon' | 'text' | 'both';
  size?: '1' | '2';
  // Accepted for call-site compatibility but no longer selects art: the animated
  // rabbit (one PNG per tunnel state) was replaced by the single fixed Warren
  // mark. Kept so the header's existing wiring does not break.
  state?: LogoState;
  // Over the bright scenery the light art loses contrast, so some screens render
  // it dark. Defaults to the light-on-charcoal colour.
  wordmarkTone?: 'light' | 'dark';
}

// Retained for the header's getLogoStateByTunnelState wiring; no longer maps to a
// mark (see LogoProps.state).
export type LogoState = 'exposed' | 'hidden' | 'blocked';

const iconSizes = {
  '1': 40,
  '2': 106,
};

const textFontSizes = {
  '1': 26,
  '2': 48,
};

const tint = (tone: 'light' | 'dark') =>
  // True black over the bright sky (the charcoal darkBlue reads washed-out
  // there); the light-on-charcoal grey everywhere else.
  tone === 'dark' ? colors.black : colors.whiteOnDarkBlue80;

// Both marks are single-colour vectors painted through a CSS mask (not <img>) so
// the tone guard can recolour them per screen; an <img> cannot inherit a colour.
// The near-square ears art is the standalone "W".
const Icon = ({ size, tone }: { size: number; tone: 'light' | 'dark' }) => (
  <span
    role="img"
    aria-label="Warren"
    style={{
      display: 'inline-block',
      height: size,
      width: size,
      backgroundColor: tint(tone),
      WebkitMaskImage: 'url(assets/images/warren-mark.svg)',
      maskImage: 'url(assets/images/warren-mark.svg)',
      WebkitMaskRepeat: 'no-repeat',
      maskRepeat: 'no-repeat',
      WebkitMaskPosition: 'center',
      maskPosition: 'center',
      WebkitMaskSize: 'contain',
      maskSize: 'contain',
    }}
  />
);

// The wordmark is one vector (assets/images/warren-wordmark.svg): the ears mark
// as the "W", then "arren" set in Coliner Light. It is painted through a CSS
// mask rather than an <img> so the tone guard can still recolour it per screen
// (an <img> cannot inherit currentColor). fontSize keeps the same meaning as the
// old text wordmark: the cap band is ~fontSize, the ears rise to ~1.35x above it.
const WORDMARK_ASPECT = 3203 / 720;
const Wordmark = ({ fontSize, tone = 'light' }: { fontSize: number; tone?: 'light' | 'dark' }) => {
  const height = Math.round(fontSize * 1.35);
  return (
    <span
      role="img"
      aria-label="Warren"
      style={{
        display: 'inline-block',
        height,
        width: Math.round(height * WORDMARK_ASPECT),
        backgroundColor: tint(tone),
        WebkitMaskImage: 'url(assets/images/warren-wordmark.svg)',
        maskImage: 'url(assets/images/warren-wordmark.svg)',
        WebkitMaskRepeat: 'no-repeat',
        maskRepeat: 'no-repeat',
        WebkitMaskPosition: 'left center',
        maskPosition: 'left center',
        WebkitMaskSize: 'contain',
        maskSize: 'contain',
      }}
    />
  );
};

export const Logo = ({
  variant = 'icon',
  size: sizeProp = '1',
  wordmarkTone = 'light',
}: LogoProps) => {
  switch (variant) {
    case 'icon':
      return <Icon size={iconSizes[sizeProp]} tone={wordmarkTone} />;
    // The Warren wordmark already embeds the mark as its "W", so "both" and
    // "text" render the same single lockup, with no separate icon beside it.
    case 'text':
    case 'both':
      return <Wordmark fontSize={textFontSizes[sizeProp]} tone={wordmarkTone} />;
  }
};
