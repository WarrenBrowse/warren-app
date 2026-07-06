import { colors } from '../../foundations';
import { FontFamilies } from '../../foundations/variables';
import { Flex } from '../flex';

export interface LogoProps {
  variant?: 'icon' | 'text' | 'both';
  size?: '1' | '2';
  state?: LogoState;
  // Wordmark text colour only. The guard below concerns the MARK art, not the
  // wordmark: over the bright scenery the light wordmark loses contrast, so the
  // main header renders it dark. Defaults to the light-on-charcoal colour.
  wordmarkTone?: 'light' | 'dark';
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

// The dark rabbit IS the brand (poka, 2026-07-04): one mark per state,
// identical on every screen and background. There is deliberately NO
// tone/color knob on this component, so a per-screen variant of the
// mark is impossible by construction; introducing one again requires
// changing this API and shipping new art, both loud in review.
const markAssets: Record<LogoState, string> = {
  exposed: 'logo-rabbit',
  hidden: 'logo-ears',
  // TODO: give the kill-switch "internet blocked" state its own mark (per the
  // art direction, an ocre rabbit with crossed-out eyes). Until that art exists
  // it falls back to the exposed face.
  blocked: 'logo-rabbit',
};

// All mark PNGs share this canvas and a bottom-anchored burrow, so every state
// renders the exact same box (the hole stays put; only the rabbit ducks in/out).
const MARK_ASPECT = 968 / 687;

const Mark = ({ size, state }: { size: number; state: LogoState }) => (
  <img
    src={`assets/images/${markAssets[state]}.png`}
    height={size}
    width={Math.round(size * MARK_ASPECT)}
    alt="Warren"
    draggable={false}
  />
);

const Wordmark = ({ fontSize, tone = 'light' }: { fontSize: number; tone?: 'light' | 'dark' }) => (
  <span
    style={{
      fontFamily: FontFamilies.nunito,
      fontWeight: 900,
      fontSize,
      lineHeight: 1,
      // True black (not the charcoal darkBlue, which reads washed-out over the
      // bright sky), matching the black rabbit mark and the flat header icons.
      color: tone === 'dark' ? colors.black : colors.whiteOnDarkBlue80,
      whiteSpace: 'nowrap',
    }}>
    <span style={{ fontSize: '1.35em' }}>W</span>
    <span style={{ marginLeft: '-0.28em', letterSpacing: '0.01em' }}>ARREN</span>
  </span>
);

export const Logo = ({
  variant = 'icon',
  size: sizeProp = '1',
  state = 'exposed',
  wordmarkTone = 'light',
}: LogoProps) => {
  switch (variant) {
    case 'icon':
      return <Mark size={iconSizes[sizeProp]} state={state} />;
    case 'text':
      return <Wordmark fontSize={textFontSizes[sizeProp]} tone={wordmarkTone} />;
    case 'both':
      return (
        <Flex alignItems="center" gap="tiny">
          <Mark size={iconSizes[sizeProp]} state={state} />
          <Wordmark fontSize={textFontSizes[sizeProp]} tone={wordmarkTone} />
        </Flex>
      );
  }
};
