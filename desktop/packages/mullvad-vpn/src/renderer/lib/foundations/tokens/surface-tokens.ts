import type { Theme } from '../../../../shared/theme';

// The surfaces that float over the scenery on the connect screen (the connection
// card, the BETA banner, the feature chips), in both themes. They are opaque:
// a translucent surface let each landscape tint it, so the same card read
// olive on one country and slate on the next.
//
// Unlike the neutral palette in color-tokens.ts, these surfaces are warm on
// purpose: they are the paper the art direction frames the scenery with, a
// warm charcoal in the dark theme and a cream in the light one.
//
// The action fills (connect, disconnect, cancel) carry white text and are the
// same in both themes; they sit in the tables so that a theme is one complete
// set a client can take as a whole.
const dark = {
  card: 'rgb(40, 38, 35)',
  line: 'rgb(69, 66, 60)',
  shadowSoft: 'rgba(0, 0, 0, 0.35)',
  shadowStrong: 'rgba(0, 0, 0, 0.45)',
  pill: 'rgb(217, 164, 65)',
  pillText: 'rgb(35, 26, 6)',
  text: 'rgb(242, 239, 230)',
  textSecondary: 'rgb(214, 210, 200)',
  textMuted: 'rgb(181, 176, 164)',
  button: 'rgb(58, 56, 52)',
  buttonHover: 'rgb(69, 66, 61)',
  buttonPressed: 'rgb(49, 47, 43)',
  buttonLine: 'rgb(85, 82, 75)',
  exposed: 'rgb(240, 138, 110)',
  exposedWell: 'rgb(74, 42, 34)',
  connecting: 'rgb(240, 163, 96)',
  connectingWell: 'rgb(74, 52, 28)',
  protected: 'rgb(159, 208, 126)',
  protectedWell: 'rgb(36, 58, 31)',
  connect: 'rgb(63, 107, 46)',
  connectHover: 'rgb(72, 122, 53)',
  connectPressed: 'rgb(53, 90, 39)',
  disconnect: 'rgb(168, 56, 31)',
  disconnectHover: 'rgb(184, 65, 38)',
  disconnectPressed: 'rgb(143, 47, 26)',
  cancel: 'rgb(160, 88, 24)',
  cancelHover: 'rgb(176, 98, 28)',
  cancelPressed: 'rgb(138, 76, 21)',
  actionText: 'rgb(255, 255, 255)',
};

export type SurfaceToken = keyof typeof dark;

export const surfaceTokens: Record<Theme, Record<SurfaceToken, string>> = {
  dark,
  light: {
    card: 'rgb(247, 241, 227)',
    line: 'rgb(217, 205, 178)',
    shadowSoft: 'rgba(60, 50, 30, 0.18)',
    shadowStrong: 'rgba(60, 50, 30, 0.28)',
    pill: 'rgb(122, 84, 18)',
    pillText: 'rgb(255, 255, 255)',
    text: 'rgb(42, 40, 34)',
    textSecondary: 'rgb(74, 70, 61)',
    textMuted: 'rgb(92, 87, 76)',
    button: 'rgb(235, 227, 208)',
    buttonHover: 'rgb(228, 218, 195)',
    buttonPressed: 'rgb(220, 208, 182)',
    buttonLine: 'rgb(207, 194, 165)',
    exposed: 'rgb(163, 50, 28)',
    exposedWell: 'rgb(243, 217, 207)',
    connecting: 'rgb(138, 75, 15)',
    connectingWell: 'rgb(243, 224, 200)',
    protected: 'rgb(47, 106, 42)',
    protectedWell: 'rgb(216, 232, 204)',
    connect: 'rgb(63, 107, 46)',
    connectHover: 'rgb(72, 122, 53)',
    connectPressed: 'rgb(53, 90, 39)',
    disconnect: 'rgb(168, 56, 31)',
    disconnectHover: 'rgb(184, 65, 38)',
    disconnectPressed: 'rgb(143, 47, 26)',
    cancel: 'rgb(160, 88, 24)',
    cancelHover: 'rgb(176, 98, 28)',
    cancelPressed: 'rgb(138, 76, 21)',
    actionText: 'rgb(255, 255, 255)',
  },
};
