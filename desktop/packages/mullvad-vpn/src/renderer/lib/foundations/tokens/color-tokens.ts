// Warren dark palette.
//
// HARD RULE (learned the hard way): the NEUTRALS are truly NEUTRAL grey
// (backgrounds, surfaces, text/icon greys, the map). They carry essentially no
// hue. If the neutrals lean warm, the whole screen reads as a yellow/sepia
// filter because there is no neutral reference point anywhere. ALL of the
// warmth/identity comes from the ACCENTS only (olive = connected, terracotta =
// disconnected, ocre = brand) and from the logo, which cover small areas and
// therefore read as colour, not as a wash. The lightness ladder of each group is
// preserved so contrast relationships hold.
//
// Reference accent palette (art direction "L'univers de Bula"):
//   Vert olive #8C8A5B · Terre #A9784E · Ocre #C2913C · Creme #F5ECDA
export const colorTokens = {
  // True near-white (neutral, NOT cream).
  white: 'rgb(247, 247, 248)',
  whiteAlpha80: 'rgba(247, 247, 248, 0.8)',
  whiteAlpha60: 'rgba(247, 247, 248, 0.6)',
  whiteAlpha40: 'rgba(247, 247, 248, 0.4)',
  whiteAlpha20: 'rgba(247, 247, 248, 0.2)',

  black: 'rgb(0, 0, 0)',
  blackAlpha80: 'rgba(0, 0, 0, 0.8)',
  blackAlpha60: 'rgba(0, 0, 0, 0.6)',
  blackAlpha50: 'rgba(0, 0, 0, 0.5)',
  blackAlpha40: 'rgba(0, 0, 0, 0.4)',

  // Disconnected / unsecured / error: terracotta (accent, saturated).
  red: 'rgb(202, 76, 56)',
  newRed: 'rgb(214, 96, 70)',
  redAlpha40: 'rgba(202, 76, 56, 0.4)',
  red80: 'rgb(176, 68, 52)',
  red40: 'rgb(100, 50, 44)',

  // Connected / secured / success: a clear, confident olive-green (kept vivid
  // enough to read as "secure GO" and to contrast strongly with the red state).
  green: 'rgb(110, 162, 78)',
  greenAlpha40: 'rgba(110, 162, 78, 0.4)',
  green80: 'rgb(94, 142, 66)',
  green40: 'rgb(58, 86, 48)',

  // Connecting / in-progress: a distinct orange sitting between the exposed red
  // and the secured green, so the transitional state reads as its own phase
  // rather than a washed-out red.
  orange: 'rgb(224, 122, 40)',
  orangeAlpha40: 'rgba(224, 122, 40, 0.4)',
  orange80: 'rgb(198, 106, 34)',

  // Warning + brand warm accents (used sparingly, kept saturated).
  yellow: 'rgb(202, 150, 60)', // ocre
  fur: 'rgb(208, 150, 64)', // ocre / beige, Bula's fur
  nose: 'rgb(232, 200, 150)', // soft apricot

  // "blue" is the default/primary interactive surface (buttons, raised cells).
  // Neutral grey, only a hair warm so it never casts.
  blue: 'rgb(74, 72, 70)',
  darkBlue: 'rgb(31, 31, 32)', // main app background: neutral charcoal

  dark: 'rgb(66, 65, 64)',
  darkerBlue50: 'rgb(26, 26, 27)',
  darkerBlue50Alpha80: 'rgba(26, 26, 27, 0.8)',
  darkerBlue10: 'rgb(18, 18, 19)',
  darkerBlue10Alpha80: 'rgba(18, 18, 19, 0.8)',
  darkerBlue10Alpha40: 'rgba(18, 18, 19, 0.4)',

  // Neutral grey surface ladder (lightest -> darkest).
  blue10: 'rgb(42, 41, 40)',
  blue20: 'rgb(48, 47, 46)',
  blue40: 'rgb(56, 55, 53)',
  blue50: 'rgb(64, 62, 60)',
  blue60: 'rgb(72, 70, 68)',
  blue80: 'rgb(80, 78, 75)',

  // Text / icon ladder on the dark background (darkest -> near-white), neutral.
  whiteOnDarkBlue5: 'rgb(50, 50, 51)',
  whiteOnDarkBlue10: 'rgb(66, 66, 67)',
  whiteOnDarkBlue20: 'rgb(92, 92, 93)',
  whiteOnDarkBlue40: 'rgb(134, 134, 135)',
  whiteOnDarkBlue50: 'rgb(158, 158, 159)',
  whiteOnDarkBlue60: 'rgb(186, 186, 187)',
  whiteOnDarkBlue80: 'rgb(228, 228, 229)',

  // Text / icon ladder on the lighter grey surface, neutral.
  whiteOnBlue5: 'rgb(76, 75, 73)',
  whiteOnBlue10: 'rgb(88, 87, 85)',
  whiteOnBlue20: 'rgb(112, 111, 108)',
  whiteOnBlue40: 'rgb(150, 149, 146)',
  whiteOnBlue50: 'rgb(174, 173, 170)',
  whiteOnBlue60: 'rgb(198, 197, 194)',
  whiteOnBlue80: 'rgb(226, 226, 224)',

  // Warm "paper" surface (mnemonic grid only): a deliberate small warm accent.
  chalk: 'rgb(244, 240, 232)',
  chalkAlpha80: 'rgba(244, 240, 232, 0.8)',
  chalkAlpha40: 'rgba(244, 240, 232, 0.4)',
  chalk80: 'rgb(236, 226, 204)',

  transparent: 'transparent',
} as const;
