import { colors } from '../../foundations';
import { FontFamilies } from '../../foundations/variables';
import { Flex } from '../flex';

export interface LogoProps {
  variant?: 'icon' | 'text' | 'both';
  size?: '1' | '2';
  // 'dark' is used on the coloured app header (green/red), 'light' on dark
  // surfaces (launch / error screens).
  tone?: 'light' | 'dark';
  // Which mark Bula shows, driven by the connection state (see markAssets).
  state?: LogoState;
}

// 'exposed'  : Bula's masked face is out of the burrow (disconnected).
// 'hidden'   : Bula is safe in the burrow, only the ears show (connected).
// 'blocked'  : internet blocked by the kill switch (future state).
export type LogoState = 'exposed' | 'hidden' | 'blocked';

const iconSizes = {
  '1': 40,
  '2': 106,
};

// The wordmark is set in Nunito Black (heavy + rounded, echoing Bula). Sizes are
// tuned to feel fat and prominent while still fitting the 68px header (size 1).
const textFontSizes = {
  '1': 30,
  '2': 56,
};

// Per state, which mark file Bula shows. The black master sits on coloured/light
// surfaces; the cream variant is tuned for dark surfaces (the dark background
// shows through the transparent cut-outs: mask, eyes, inner ears). All four PNGs
// share the same 968x784 canvas, so the ears stay put and only the head/body
// ducks in or out of the hole as the state changes.
const markAssets: Record<LogoState, { light: string; dark: string }> = {
  exposed: { dark: 'logo-rabbit', light: 'logo-rabbit-cream' },
  hidden: { dark: 'logo-ears', light: 'logo-ears-cream' },
  // TODO: give the kill-switch "internet blocked" state its own mark (per the
  // art direction, an ocre rabbit with crossed-out eyes). Until that art exists
  // it falls back to the exposed face.
  blocked: { dark: 'logo-rabbit', light: 'logo-rabbit-cream' },
};

const Mark = ({
  size,
  tone,
  state,
}: {
  size: number;
  tone: 'light' | 'dark';
  state: LogoState;
}) => (
  <img
    src={`assets/images/${markAssets[state][tone]}.png`}
    height={size}
    alt="Warren"
    draggable={false}
  />
);

const Wordmark = ({ fontSize, tone }: { fontSize: number; tone: 'light' | 'dark' }) => (
  <span
    style={{
      fontFamily: FontFamilies.nunito,
      fontWeight: 900,
      fontSize,
      letterSpacing: '0.01em',
      lineHeight: 1,
      color: tone === 'dark' ? colors.darkerBlue10 : colors.whiteOnDarkBlue80,
      whiteSpace: 'nowrap',
    }}>
    WARREN
  </span>
);

export const Logo = ({
  variant = 'icon',
  size: sizeProp = '1',
  tone = 'light',
  state = 'exposed',
}: LogoProps) => {
  switch (variant) {
    case 'icon':
      return <Mark size={iconSizes[sizeProp]} tone={tone} state={state} />;
    case 'text':
      return <Wordmark fontSize={textFontSizes[sizeProp]} tone={tone} />;
    case 'both': {
      // Lift the mark and drop the wordmark a touch so the logo reads slightly
      // higher than the text. This brings the low-sitting "hidden" ears up to
      // WARREN's height while letting the "exposed" rabbit peek just above it.
      // Both states keep the exact same mark size: this is only a visual offset
      // (transform, no layout reflow), so nothing shifts on a state change.
      const lift = Math.round(iconSizes[sizeProp] * 0.12);
      const drop = Math.round(iconSizes[sizeProp] * 0.05);
      return (
        <Flex alignItems="center" gap="small">
          <span style={{ display: 'flex', transform: `translateY(-${lift}px)` }}>
            <Mark size={iconSizes[sizeProp]} tone={tone} state={state} />
          </span>
          <span style={{ display: 'inline-flex', transform: `translateY(${drop}px)` }}>
            <Wordmark fontSize={textFontSizes[sizeProp]} tone={tone} />
          </span>
        </Flex>
      );
    }
  }
};
