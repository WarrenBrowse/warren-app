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
  '1': 26,
  '2': 48,
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
      lineHeight: 1,
      color: tone === 'dark' ? colors.darkerBlue10 : colors.whiteOnDarkBlue80,
      whiteSpace: 'nowrap',
    }}>
    {/* Oversized initial "W" (drop cap) echoing Bula's rounded forms. "ARREN" is
        pulled in under it with a small negative margin, leaving only a slight
        space. Sizes/margins are in em so the lettering scales as one unit between
        the header and the launch screen, and the two spans share the text baseline
        so the W and ARREN stay bottom-aligned. */}
    <span style={{ fontSize: '1.35em' }}>W</span>
    <span style={{ marginLeft: '-0.28em', letterSpacing: '0.01em' }}>ARREN</span>
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
    case 'both':
      // Align on the baseline so the bottom of the mark (the static burrow line,
      // shared by every state's PNG) sits exactly on WARREN's baseline. The mark
      // is taller than the text, so it rises above the wordmark while staying
      // bottom-aligned with it, no matter the connection state.
      return (
        // The mark is taller than the header icons, so when the row is centred
        // the logo + wordmark sit a touch lower than the account/settings icons.
        // Lift the block so its bottom (burrow + WARREN baseline) lines up with
        // the bottom of those icons.
        <Flex alignItems="baseline" gap="tiny" style={{ transform: 'translateY(-7px)' }}>
          <Mark size={iconSizes[sizeProp]} tone={tone} state={state} />
          <Wordmark fontSize={textFontSizes[sizeProp]} tone={tone} />
        </Flex>
      );
  }
};
