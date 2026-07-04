import { colors } from '../../foundations';
import { FontFamilies } from '../../foundations/variables';
import { Flex } from '../flex';

export interface LogoProps {
  variant?: 'icon' | 'text' | 'both';
  size?: '1' | '2';
  tone?: 'light' | 'dark';
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

const textFontSizes = {
  '1': 26,
  '2': 48,
};

const markAssets: Record<LogoState, { light: string; dark: string }> = {
  exposed: { dark: 'logo-rabbit', light: 'logo-rabbit-cream' },
  hidden: { dark: 'logo-ears', light: 'logo-ears-cream' },
  // TODO: give the kill-switch "internet blocked" state its own mark (per the
  // art direction, an ocre rabbit with crossed-out eyes). Until that art exists
  // it falls back to the exposed face.
  blocked: { dark: 'logo-rabbit', light: 'logo-rabbit-cream' },
};

// All mark PNGs share this canvas and a bottom-anchored burrow, so every state
// renders the exact same box (the hole stays put; only the rabbit ducks in/out).
const MARK_ASPECT = 968 / 687;

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
    width={Math.round(size * MARK_ASPECT)}
    alt="Warren"
    draggable={false}
  />
);

const Wordmark = ({ fontSize }: { fontSize: number }) => (
  <span
    style={{
      fontFamily: FontFamilies.nunito,
      fontWeight: 900,
      fontSize,
      lineHeight: 1,
      color: colors.whiteOnDarkBlue80,
      whiteSpace: 'nowrap',
    }}>
    <span style={{ fontSize: '1.35em' }}>W</span>
    <span style={{ marginLeft: '-0.28em', letterSpacing: '0.01em' }}>ARREN</span>
  </span>
);

// The dark mark IS the brand (poka, 2026-07-04): it stays dark on
// every background, colored or grey, even next to the white wordmark.
// The cream variant is kept only for future art-direction needs.
export const Logo = ({
  variant = 'icon',
  size: sizeProp = '1',
  tone = 'dark',
  state = 'exposed',
}: LogoProps) => {
  switch (variant) {
    case 'icon':
      return <Mark size={iconSizes[sizeProp]} tone={tone} state={state} />;
    case 'text':
      return <Wordmark fontSize={textFontSizes[sizeProp]} />;
    case 'both':
      return (
        <Flex alignItems="center" gap="tiny">
          <Mark size={iconSizes[sizeProp]} tone={tone} state={state} />
          <Wordmark fontSize={textFontSizes[sizeProp]} />
        </Flex>
      );
  }
};
