// The geometry of the home screen, in CSS pixels, which Android reads as dp
// (sp for a key ending in `size`) and iOS as points through design-tokens.json.
// Every home component takes its measures from here and nowhere else, so the
// generator copies values instead of scraping them out of styled blocks.
//
// Unit by key, as scripts/design-tokens/lib.mjs reads them: a key ending in
// `size` is text (sp), in `weight` a font weight, in `em` a fraction of the font
// size, in `stop` a fraction of a length, in `ms` a duration; anything else is a
// length (dp).
export const homeTokens = {
  hero: {
    // The header floats over the art; nothing important may sit above
    // headerInset + toolbarHeight + clearMargin.
    headerInset: 14,
    clearMargin: 6,
    // The mockup's logo image is 40 high with a transparent margin; the bare
    // wordmark covers the same ink (113 by 37 on the 380px window) at 37,
    // inset by that margin.
    logoHeight: 37,
    logoInset: 6,
    // The page colour washes the top of the art down to this fraction, so the
    // toolbar and the logo read on the brightest sky.
    scrimStop: 0.34,
    glowWidth: 300,
    glowHeight: 150,
    frameMs: 900,
    crossfadeMs: 700,
    bulaMs: 550,
    blurRadius: 14,
  },
  toolbar: {
    height: 48,
    radius: 14,
    paddingHorizontal: 6,
    button: 38,
    icon: 23,
    // The unread dot, inset from the button's top right corner.
    badge: 8,
    badgeInset: 7,
    badgeRing: 2,
    blur: 10,
  },
  // The banners hung under the toolbar, on its right edge.
  notice: {
    marginTop: 10,
    maxWidth: 250,
    radius: 14,
    blur: 12,
    gap: 10,
    paddingVertical: 9,
    paddingStart: 12,
    paddingEnd: 10,
    // A banner ending on a button sits tighter around it.
    buttonInset: 8,
    icon: 20,
    close: 18,
    titleSize: 13,
    titleWeight: 600,
    bodySize: 11,
    bodyGap: 1,
    buttonHeight: 30,
    buttonPaddingHorizontal: 12,
    buttonRadius: 9,
    buttonTextSize: 12,
    buttonTextWeight: 600,
  },
  panel: {
    paddingHorizontal: 22,
    // How far the status block rides up over the fade.
    overlap: 18,
    gap: 4,
  },
  pill: {
    paddingVertical: 7,
    paddingStart: 10,
    paddingEnd: 13,
    gap: 7,
    icon: 16,
    textSize: 13,
    textWeight: 700,
  },
  title: {
    marginTop: 8,
    textSize: 26,
    textWeight: 600,
    letterSpacingEm: -0.01,
  },
  subtitle: {
    marginTop: 2,
    paddingVertical: 4,
    lineHeight: 20,
    textSize: 13,
    monoSize: 12,
  },
  // The exit line under a protected title: name, load, people, and the chevron
  // that opens the details. The whole row is the button, its hover area
  // reaching out past the text.
  exitRow: {
    marginTop: 2,
    outset: 8,
    paddingVertical: 4,
    radius: 8,
    gap: 10,
    textSize: 12,
    ring: 11,
    ringHole: 3,
    person: 15,
    partGap: 4,
    chevron: 20,
    chevronMs: 300,
  },
  // The connection details, opened in place under the exit line while the
  // hero shrinks to make room.
  details: {
    marginTop: 6,
    radius: 12,
    paddingVertical: 12,
    paddingHorizontal: 14,
    columnGap: 16,
    rowGap: 7,
    textSize: 12,
    valueWeight: 500,
    monoSize: 11,
    dividerMargin: 2,
    chipSize: 10,
    chipWeight: 600,
    chipPaddingVertical: 1,
    chipPaddingHorizontal: 6,
  },
  controls: {
    marginTop: 16,
    gap: 8,
    rowHeight: 46,
    radius: 10,
    thumb: 28,
    // The hairline that keeps a white flag off a white field.
    thumbLine: 1,
    paddingStart: 10,
    paddingEnd: 14,
    rowGap: 10,
    labelSize: 15,
    labelWeight: 500,
    changeSize: 13,
    buttonHeight: 50,
    buttonTextSize: 15,
    buttonTextWeight: 600,
    outlineWidth: 1,
  },
  chips: {
    marginTop: 12,
    // Fixed, so the row reserved under the controls is exactly one chip tall.
    height: 23,
    gap: 6,
    textSize: 11,
    paddingVertical: 3,
    paddingHorizontal: 9,
    borderWidth: 1,
  },
  footer: {
    paddingVertical: 12,
    paddingHorizontal: 22,
    textSize: 12,
    icon: 16,
    gap: 6,
  },
} as const;

// The span the framing must keep clear of the header, from the hero top.
export const homeClearTop =
  homeTokens.hero.headerInset + homeTokens.toolbar.height + homeTokens.hero.clearMargin;
