// The design-token generator: reads the desktop foundation token files and the
// handful of desktop component sources the connect screen is built from, and
// produces the checked-in `design-tokens.json` plus the generated Kotlin
// `DesignTokens.kt`. The desktop is the source of truth for every value here;
// Android consumes the generated object, never a hand copy.
//
// Two gates hold the three artefacts together, the way the scenery assets are
// pinned: `test/unit/design-tokens.spec.ts` (Node only) regenerates everything
// in memory and fails when the JSON or the Kotlin is stale against the desktop
// sources, and `DesignTokensGateTest` (JVM) hashes the JSON and fails when the
// Kotlin was generated from another revision of it.

import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import path from 'node:path';

export const JSON_PATH = 'design-tokens.json';
export const KOTLIN_PATH =
  'android/lib/ui/theme/src/main/kotlin/com/warrenbrowse/vpn/lib/ui/theme/tokens/DesignTokens.kt';

const RENDERER = 'desktop/packages/mullvad-vpn/src/renderer';
const TOKENS = `${RENDERER}/lib/foundations/tokens`;
const CONNECTION_PANEL = `${RENDERER}/components/views/main/components/connection-panel`;

const SOURCES = {
  colors: `${TOKENS}/color-tokens.ts`,
  radius: `${TOKENS}/radius-tokens.ts`,
  spacing: `${TOKENS}/spacing-tokens.ts`,
  typography: `${TOKENS}/typography-tokens.ts`,
  icon: `${RENDERER}/lib/components/icon/Icon.tsx`,
  connectionPanel: `${CONNECTION_PANEL}/ConnectionPanel.tsx`,
  connectionStatus: `${CONNECTION_PANEL}/components/connection-status/ConnectionStatus.tsx`,
  featureIndicator: `${RENDERER}/lib/components/feature-indicator/FeatureIndicator.tsx`,
  countryFlag: `${RENDERER}/components/CurrentCountryFlag.tsx`,
  footer: `${RENDERER}/components/app-main-header/components/AppMainFooter.tsx`,
  notificationBanner: `${RENDERER}/components/NotificationBanner.tsx`,
  scenery: `${RENDERER}/components/CountryBackdrop/index.tsx`,
  navigation: `${RENDERER}/lib/transition-hooks.ts`,
};

function read(repoRoot, relative) {
  return readFileSync(path.join(repoRoot, relative), 'utf8');
}

/**
 * The text of one styled block: from `const <name>` to the next top-level
 * declaration. Values are matched inside the block so a `gap: '12px'` of one
 * component can never be mistaken for another's.
 */
function block(text, name) {
  const start = text.indexOf(`const ${name}`);
  if (start < 0) throw new Error(`block ${name} not found`);
  const rest = text.slice(start + 1);
  const end = rest.search(/\n(const|export|function|interface|type) /);
  return end < 0 ? text.slice(start) : text.slice(start, start + 1 + end);
}

function match(text, regex, what) {
  const m = regex.exec(text);
  if (!m) throw new Error(`${what}: ${regex} did not match`);
  return m[1];
}

const num = (text, regex, what) => Number(match(text, regex, what));

/** `rgb(1, 2, 3)` / `rgba(1, 2, 3, 0.4)` / `transparent` to Android `#AARRGGBB`. */
export function cssColorToHex(css) {
  if (css === 'transparent') return '#00000000';
  const m = /^rgba?\((\d+),\s*(\d+),\s*(\d+)(?:,\s*([\d.]+))?\)$/.exec(css);
  if (!m) throw new Error(`unsupported colour ${css}`);
  const [r, g, b] = [m[1], m[2], m[3]].map((v) => Number(v).toString(16).padStart(2, '0'));
  const alpha = m[4] === undefined ? 1 : Number(m[4]);
  const a = Math.round(alpha * 255)
    .toString(16)
    .padStart(2, '0');
  return `#${a}${r}${g}${b}`.toUpperCase();
}

function colorTokens(text) {
  const out = {};
  for (const m of text.matchAll(/^\s+(\w+):\s*'([^']+)',/gm)) {
    out[m[1]] = cssColorToHex(m[2]);
  }
  if (Object.keys(out).length === 0) throw new Error('no colour tokens found');
  return out;
}

function pxEnum(text, what) {
  const out = {};
  for (const m of text.matchAll(/^\s+(\w+)\s*[=:]\s*'(\d+)px',?/gm)) out[m[1]] = Number(m[2]);
  if (Object.keys(out).length === 0) throw new Error(`no ${what} tokens found`);
  return out;
}

function enumBlock(text, name) {
  const start = text.indexOf(`export enum ${name}`);
  if (start < 0) throw new Error(`enum ${name} not found`);
  return text.slice(start, text.indexOf('}', start));
}

function typographyTokens(text) {
  const families = {};
  for (const m of enumBlock(text, 'FontFamilyTokens').matchAll(/^\s+(\w+)\s*=\s*'([^']+)',/gm)) {
    // The first family of a CSS stack is the face itself; the rest are the
    // browser fallbacks, which Android does not need.
    families[m[1]] = m[2].split(',')[0].replace(/"/g, '').trim();
  }
  const weights = {};
  for (const m of enumBlock(text, 'FontWeightTokens').matchAll(/^\s+(\w+)\s*=\s*(\d+),/gm)) {
    weights[m[1]] = Number(m[2]);
  }
  return {
    fontFamilies: families,
    fontWeights: weights,
    fontSizes: pxEnum(enumBlock(text, 'FontSizeTokens'), 'font size'),
    lineHeights: pxEnum(enumBlock(text, 'LineHeightTokens'), 'line height'),
  };
}

const dp = (value) => ({ value, unit: 'dp' });
const sp = (value) => ({ value, unit: 'sp' });
const ms = (value) => ({ value, unit: 'ms' });
const ratio = (value) => ({ value, unit: 'ratio' });

/** The alpha channel of a colour token, as the 0..1 fraction the CSS carried. */
function alphaOf(colors, name) {
  if (!(name in colors)) throw new Error(`colour token ${name} missing`);
  return ratio(Number((parseInt(colors[name].slice(1, 3), 16) / 255).toFixed(2)));
}

function componentTokens(repoRoot, colors, radius) {
  const panel = read(repoRoot, SOURCES.connectionPanel);
  const card = block(panel, 'StyledCard');
  const [cardPadV, cardPadH] = match(card, /padding: '(\d+px \d+px)'/, 'card padding')
    .split(' ')
    .map((v) => Number(v.replace('px', '')));
  const cardSurface = match(card, /backgroundColor: colors\.(\w+)/, 'card surface');
  const cardBorder = match(card, /border: `1px solid \$\{colors\.(\w+)\}`/, 'card border');

  const status = read(repoRoot, SOURCES.connectionStatus);
  const well = block(status, 'StyledIconWell');
  const icon = read(repoRoot, SOURCES.icon);
  const iconSizes = block(icon, 'iconSizes');
  const wellIconSize = match(status, /<Icon icon=\{eyeIcon\} color=\{colorName\} size="(\w+)"/, 'eye size');

  const chip = read(repoRoot, SOURCES.featureIndicator);
  const [chipPadV, chipPadH] = match(block(chip, 'StyledFlex'), /padding: (\d+px \d+px);/, 'chip padding')
    .split(' ')
    .map((v) => Number(v.replace('px', '')));
  const chipRadius = match(block(chip, 'StyledFeatureIndicator'), /border-radius: \$\{Radius\.(\w+)\}/, 'chip radius');
  const chipVariants = block(chip, 'styles');

  const flag = block(read(repoRoot, SOURCES.countryFlag), 'StyledFlag');
  const footer = block(read(repoRoot, SOURCES.footer), 'StyledFooter');
  const [footerPadV, footerPadH] = match(footer, /padding: (\d+px \d+px);/, 'footer padding')
    .split(' ')
    .map((v) => Number(v.replace('px', '')));

  const banner = read(repoRoot, SOURCES.notificationBanner);
  const collapsible = block(banner, 'Collapsible');
  const bannerContent = block(banner, 'Content');
  const [bannerPadTop, bannerPadEnd, , bannerPadStart] = match(
    bannerContent,
    /padding: '(\d+px \d+px \d+px \d+px)'/,
    'banner padding',
  )
    .split(' ')
    .map((v) => Number(v.replace('px', '')));
  const bannerEdgeColor = match(collapsible, /borderTop: `2px solid \$\{colors\.(\w+)\}`/, 'banner edge');

  const scenery = read(repoRoot, SOURCES.scenery);
  const scene = block(scenery, 'Scene');
  const wash = block(scenery, 'AccentWash');
  const bula = block(scenery, 'Bula');

  const nav = read(repoRoot, SOURCES.navigation);

  return {
    connectionCard: {
      paddingVertical: dp(cardPadV),
      paddingHorizontal: dp(cardPadH),
      radius: dp(num(card, /borderRadius: '(\d+)px'/, 'card radius')),
      surfaceColor: cardSurface,
      surfaceAlpha: alphaOf(colors, cardSurface),
      borderWidth: dp(1),
      borderAlpha: alphaOf(colors, cardBorder),
      railWidth: dp(num(card, /width: '(\d+)px'/, 'rail width')),
      badgeGap: dp(num(block(panel, 'StyledFeatureBadges'), /gap: '(\d+)px'/, 'badge gap')),
      badgesToCardGap: dp(num(block(panel, 'StyledOuter'), /gap: '(\d+)px'/, 'outer gap')),
      buttonGap: dp(num(block(panel, 'StyledConnectionButtonContainer'), /gap: '(\d+)px'/, 'button gap')),
      transition: ms(num(card, /transition: 'background-color (\d+)ms/, 'rail transition')),
    },
    connectionStatus: {
      rowGap: dp(num(block(status, 'StyledRow'), /gap: '(\d+)px'/, 'status gap')),
      wellSize: dp(num(well, /width: '(\d+)px'/, 'well size')),
      wellRadius: dp(num(well, /borderRadius: '(\d+)px'/, 'well radius')),
      wellFillAlpha: ratio(num(well, /backgroundColor: `color-mix\(in srgb, \$\{props\.\$accent\} (\d+)%/, 'well fill') / 100),
      wellBorderAlpha: ratio(num(well, /border: `1px solid color-mix\(in srgb, \$\{props\.\$accent\} (\d+)%/, 'well border') / 100),
      wellTransition: ms(num(well, /transition: 'background-color (\d+)ms/, 'well transition')),
      iconSize: dp(num(iconSizes, new RegExp(`${wellIconSize}: (\\d+),`), 'icon size')),
      titleSize: sp(num(block(status, 'StyledTitle'), /fontSize: '(\d+)px'/, 'title size')),
      titleLineHeight: sp(num(block(status, 'StyledTitle'), /lineHeight: '(\d+)px'/, 'title line height')),
      subtitleSize: sp(num(block(status, 'StyledSubtitle'), /fontSize: '(\d+)px'/, 'subtitle size')),
      subtitleLineHeight: sp(num(block(status, 'StyledSubtitle'), /lineHeight: '(\d+)px'/, 'subtitle line height')),
      subtitleAlpha: alphaOf(colors, match(block(status, 'StyledSubtitle'), /color: colors\.(\w+)/, 'subtitle colour')),
    },
    featureChip: {
      paddingVertical: dp(chipPadV),
      paddingHorizontal: dp(chipPadH),
      radius: dp(radius[chipRadius]),
      borderWidth: dp(1),
      fillColor: match(chipVariants, /primary: \{\s*backgroundColor: colors\.(\w+)/, 'chip fill'),
      borderColor: match(chipVariants, /primary: \{\s*backgroundColor: colors\.\w+,\s*borderColor: colors\.(\w+)/, 'chip border'),
      errorFillColor: match(chipVariants, /error: \{\s*backgroundColor: colors\.(\w+)/, 'chip error fill'),
      errorFillAlpha: alphaOf(colors, match(chipVariants, /error: \{\s*backgroundColor: colors\.(\w+)/, 'chip error fill')),
    },
    countryFlag: {
      size: dp(num(flag, /width: (\d+)px;/, 'flag size')),
      borderWidth: dp(num(flag, /border: (\d+)px solid/, 'flag border')),
      borderAlpha: alphaOf(colors, match(flag, /border: \d+px solid \$\{colors\.(\w+)\}/, 'flag border colour')),
    },
    footer: {
      paddingVertical: dp(footerPadV),
      paddingHorizontal: dp(footerPadH),
      surfaceAlpha: alphaOf(colors, match(footer, /background-color: \$\{colors\.(\w+)\}/, 'footer surface')),
      borderWidth: dp(num(footer, /border-top: (\d+)px solid/, 'footer border')),
      borderAlpha: alphaOf(colors, match(footer, /border-top: \d+px solid \$\{colors\.(\w+)\}/, 'footer border colour')),
    },
    notificationBanner: {
      maxWidth: dp(num(collapsible, /maxWidth: '(\d+)px'/, 'banner width')),
      radius: dp(num(collapsible, /borderRadius: '(\d+)px'/, 'banner radius')),
      edgeWidth: dp(num(collapsible, /borderTop: `(\d+)px solid/, 'banner edge')),
      edgeColor: bannerEdgeColor,
      surfaceAlpha: alphaOf(colors, match(collapsible, /backgroundColor: colors\.(\w+)/, 'banner surface')),
      marginTop: dp(num(collapsible, /margin: '(\d+)px \d+px 0 auto'/, 'banner margin top')),
      marginEnd: dp(num(collapsible, /margin: '\d+px (\d+)px 0 auto'/, 'banner margin end')),
      paddingVertical: dp(bannerPadTop),
      paddingStart: dp(bannerPadStart),
      paddingEnd: dp(bannerPadEnd),
      elevation: dp(num(collapsible, /boxShadow: '0 (\d+)px/, 'banner shadow')),
      transition: ms(num(banner, /transition=\{\{ duration: ([\d.]+) \}\}/, 'banner transition') * 1000),
    },
    scenery: {
      blurRadius: dp(num(scene, /'blur\((\d+)px\) brightness/, 'scenery blur')),
      connectingBrightness: ratio(num(scene, /brightness\(([\d.]+)\)' : /, 'scenery brightness')),
      connectingZoom: ratio(num(scene, /'scale\(([\d.]+)\)' : /, 'scenery zoom')),
      blurTransition: ms(num(scene, /filter (\d+)ms/, 'blur transition')),
      zoomTransition: ms(num(scene, /transform (\d+)ms/, 'zoom transition')),
      crossfade: ms(num(block(scenery, 'FrontLandscape'), /warren-scenery-fade (\d+)ms/, 'crossfade')),
      bulaTransition: ms(num(bula, /opacity (\d+)ms/, 'bula transition')),
      bulaHideDrop: ratio(num(bula, /props\.\$visible \? 0 : (\d+)\)\}%/, 'bula drop') / 100),
      washAlpha: ratio(num(wash, /opacity: ([\d.]+);/, 'wash alpha')),
      washBlend: match(wash, /mix-blend-mode: ([a-z-]+);/, 'wash blend'),
      washTopStop: ratio(num(wash, /transparent (\d+)%,\s*transparent/, 'wash top stop') / 100),
      washBottomStop: ratio(num(wash, /transparent \d+%,\s*transparent (\d+)%/, 'wash bottom stop') / 100),
      washTransition: ms(num(wash, /'background (\d+)ms/, 'wash transition')),
    },
    navigation: {
      duration: ms(num(nav, /const TRANSITION_DURATION = (\d+);/, 'nav duration')),
      pushNewFrom: ratio(num(block(nav, 'newFromTransform'), /\[TransitionType\.push\]: 'translateX\((\d+)%\)'/, 'push new') / 100),
      pushOldTo: ratio(num(block(nav, 'oldToTransform'), /\[TransitionType\.push\]: 'translateX\((-?\d+)%\)'/, 'push old') / 100),
    },
  };
}

/** Every token, exactly as `design-tokens.json` carries it. */
export function buildTokens(repoRoot) {
  const colors = colorTokens(read(repoRoot, SOURCES.colors));
  const radius = pxEnum(read(repoRoot, SOURCES.radius), 'radius');
  return {
    $comment:
      'Generated by scripts/design-tokens/gen.mjs from the desktop token sources. Do not edit: regenerate, then commit the JSON and DesignTokens.kt together.',
    sources: SOURCES,
    colors,
    radius,
    spacing: pxEnum(read(repoRoot, SOURCES.spacing), 'spacing'),
    typography: typographyTokens(read(repoRoot, SOURCES.typography)),
    components: componentTokens(repoRoot, colors, radius),
  };
}

export const renderJson = (tokens) => `${JSON.stringify(tokens, null, 2)}\n`;

export const sha256 = (text) => createHash('sha256').update(text).digest('hex');

const pascal = (name) => name.charAt(0).toUpperCase() + name.slice(1);

function kotlinValue(entry) {
  if (typeof entry === 'string') return `"${entry}"`;
  switch (entry.unit) {
    case 'dp':
      return `${entry.value}.dp`;
    case 'sp':
      return `${entry.value}.sp`;
    case 'ms':
      return `${entry.value}`;
    case 'ratio':
      return `${entry.value}f`;
    default:
      throw new Error(`unit ${entry.unit}`);
  }
}

function kotlinDecl(name, entry) {
  const value = kotlinValue(entry);
  const isConst = typeof entry === 'string' || entry.unit === 'ms' || entry.unit === 'ratio';
  return `        ${isConst ? 'const val' : 'val'} ${pascal(name)} = ${value}`;
}

/** The Kotlin object Android reads, byte for byte. */
export function renderKotlin(tokens, jsonText) {
  const lines = [];
  lines.push('// GENERATED FILE, DO NOT EDIT.');
  lines.push('//');
  lines.push('// scripts/design-tokens/gen.mjs writes this object from design-tokens.json, which');
  lines.push('// is itself derived from the desktop token sources named in the JSON. The');
  lines.push('// desktop is the source of truth for every value; DesignTokensGateTest fails');
  lines.push('// when this file was generated from another revision of the JSON, and');
  lines.push('// test/unit/design-tokens.spec.ts fails when the JSON is stale against the');
  lines.push('// desktop. Regenerate with `node scripts/design-tokens/gen.mjs`.');
  lines.push('package com.warrenbrowse.vpn.lib.ui.theme.tokens');
  lines.push('');
  lines.push('import androidx.compose.ui.graphics.Color');
  lines.push('import androidx.compose.ui.unit.dp');
  lines.push('import androidx.compose.ui.unit.sp');
  lines.push('');
  lines.push('/** SHA-256 of design-tokens.json at generation time. */');
  lines.push(`const val DESIGN_TOKENS_SHA256 = "${sha256(jsonText)}"`);
  lines.push('');
  lines.push('@Suppress("MagicNumber", "unused")');
  lines.push('object DesignTokens {');
  lines.push('    object Colors {');
  for (const [name, hex] of Object.entries(tokens.colors)) {
    lines.push(`        val ${pascal(name)} = Color(0x${hex.slice(1)})`);
  }
  lines.push('    }');
  lines.push('');
  lines.push('    object Radius {');
  for (const [name, px] of Object.entries(tokens.radius)) lines.push(`        val ${pascal(name)} = ${px}.dp`);
  lines.push('    }');
  lines.push('');
  lines.push('    object Spacing {');
  for (const [name, px] of Object.entries(tokens.spacing)) lines.push(`        val ${pascal(name)} = ${px}.dp`);
  lines.push('    }');
  lines.push('');
  lines.push('    object FontFamilies {');
  for (const [name, family] of Object.entries(tokens.typography.fontFamilies)) {
    lines.push(`        const val ${pascal(name)} = "${family}"`);
  }
  lines.push('    }');
  lines.push('');
  lines.push('    object FontWeights {');
  for (const [name, weight] of Object.entries(tokens.typography.fontWeights)) {
    lines.push(`        const val ${pascal(name)} = ${weight}`);
  }
  lines.push('    }');
  lines.push('');
  lines.push('    object FontSizes {');
  for (const [name, px] of Object.entries(tokens.typography.fontSizes)) lines.push(`        val ${pascal(name)} = ${px}.sp`);
  lines.push('    }');
  lines.push('');
  lines.push('    object LineHeights {');
  for (const [name, px] of Object.entries(tokens.typography.lineHeights)) lines.push(`        val ${pascal(name)} = ${px}.sp`);
  lines.push('    }');
  for (const [component, entries] of Object.entries(tokens.components)) {
    lines.push('');
    lines.push(`    object ${pascal(component)} {`);
    for (const [name, entry] of Object.entries(entries)) lines.push(kotlinDecl(name, entry));
    lines.push('    }');
  }
  lines.push('}');
  return `${lines.join('\n')}\n`;
}
