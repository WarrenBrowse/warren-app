// The design-token generator: reads the desktop foundation token files (the
// home geometry included) and produces the checked-in `design-tokens.json` plus
// the generated Kotlin `DesignTokens.kt`. The desktop is the source of truth for
// every value here; Android consumes the generated object, never a hand copy.
//
// `components` is the pre-redesign connect screen, frozen in
// legacy-components.json until the Android home lot stops reading it;
// `navigation` is still measured on the desktop.
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

const SOURCES = {
  colors: `${TOKENS}/color-tokens.ts`,
  surfaces: `${TOKENS}/surface-tokens.ts`,
  radius: `${TOKENS}/radius-tokens.ts`,
  spacing: `${TOKENS}/spacing-tokens.ts`,
  typography: `${TOKENS}/typography-tokens.ts`,
  home: `${TOKENS}/home-tokens.ts`,
  legacyComponents: 'scripts/design-tokens/legacy-components.json',
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
  const start = text.search(new RegExp(`const ${name}\\b`));
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

/**
 * Both themes of the connect-screen surfaces: the `const dark = { ... }` table
 * and the `light: { ... }` table, which must name the same tokens.
 */
function surfaceTokens(text) {
  const table = (start) => {
    const from = text.indexOf(start);
    if (from < 0) throw new Error(`surface table ${start} not found`);
    const body = text.slice(from, text.indexOf('}', from));
    const out = {};
    for (const m of body.matchAll(/^\s+(\w+):\s*'([^']+)',/gm)) out[m[1]] = cssColorToHex(m[2]);
    return out;
  };
  const dark = table('const dark = {');
  const light = table('light: {');
  const names = (t) => Object.keys(t).join(',');
  if (names(dark) !== names(light)) throw new Error('the two surface themes name different tokens');
  return { dark, light };
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

const ms = (value) => ({ value, unit: 'ms' });
const ratio = (value) => ({ value, unit: 'ratio' });

function navigationTokens(repoRoot) {
  const nav = read(repoRoot, SOURCES.navigation);
  return {
    duration: ms(num(nav, /const TRANSITION_DURATION = (\d+);/, 'nav duration')),
    // The slides are written for a left-to-right reading and multiplied by the sign of the
    // inline axis, so the factor is the left-to-right offset.
    pushNewFrom: ratio(num(block(nav, 'newFromTransform'), /\[TransitionType\.push\]: \(inlineSign\) => `translateX\(\$\{(-?\d+) \* inlineSign\}%\)`/, 'push new') / 100),
    pushOldTo: ratio(num(block(nav, 'oldToTransform'), /\[TransitionType\.push\]: \(inlineSign\) => `translateX\(\$\{(-?\d+) \* inlineSign\}%\)`/, 'push old') / 100),
  };
}

function componentTokens(repoRoot) {
  const legacy = JSON.parse(read(repoRoot, SOURCES.legacyComponents)).components;
  return { ...legacy, navigation: navigationTokens(repoRoot) };
}

/** The unit a home key carries, by the naming rule home-tokens.ts documents. */
export function homeUnit(key) {
  if (/(^s|S)ize$/.test(key)) return 'sp';
  if (/(^w|W)eight$/.test(key)) return 'weight';
  if (/Em$/.test(key)) return 'ratio';
  if (/(^s|S)top$/.test(key)) return 'ratio';
  if (/Ms$/.test(key)) return 'ms';
  return 'dp';
}

/**
 * home-tokens.ts is one object literal of groups of numbers. It is read as
 * data, so a value that is not a plain number is refused rather than guessed.
 */
function homeTokens(text) {
  const from = text.indexOf('export const homeTokens = {');
  if (from < 0) throw new Error('home-tokens.ts: no homeTokens object');
  const body = text.slice(from, text.indexOf('} as const;', from));
  const out = {};
  let group = null;
  for (const line of body.split('\n').slice(1)) {
    const code = line.replace(/\/\/.*$/, '').trim();
    if (!code) continue;
    let m;
    if ((m = /^(\w+): \{$/.exec(code))) {
      group = m[1];
      out[group] = {};
    } else if (code === '},') {
      group = null;
    } else if ((m = /^(\w+): (-?\d+(?:\.\d+)?),$/.exec(code)) && group) {
      out[group][m[1]] = { value: Number(m[2]), unit: homeUnit(m[1]) };
    } else {
      throw new Error(`home-tokens.ts: cannot read "${code}"`);
    }
  }
  if (Object.keys(out).length === 0) throw new Error('home-tokens.ts: no groups');
  return out;
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
    surfaces: surfaceTokens(read(repoRoot, SOURCES.surfaces)),
    home: homeTokens(read(repoRoot, SOURCES.home)),
    components: componentTokens(repoRoot),
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
    case 'weight':
      return `${entry.value}`;
    default:
      throw new Error(`unit ${entry.unit}`);
  }
}

function kotlinDecl(name, entry) {
  const value = kotlinValue(entry);
  const isConst =
    typeof entry === 'string' || entry.unit === 'ms' || entry.unit === 'ratio' || entry.unit === 'weight';
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
  lines.push('');
  lines.push('    object Surfaces {');
  for (const [theme, table] of Object.entries(tokens.surfaces)) {
    lines.push(`        object ${pascal(theme)} {`);
    for (const [name, hex] of Object.entries(table)) {
      lines.push(`            val ${pascal(name)} = Color(0x${hex.slice(1)})`);
    }
    lines.push('        }');
  }
  lines.push('    }');
  lines.push('');
  lines.push('    object Home {');
  for (const [group, entries] of Object.entries(tokens.home)) {
    lines.push(`        object ${pascal(group)} {`);
    for (const [name, entry] of Object.entries(entries)) lines.push(`    ${kotlinDecl(name, entry)}`);
    lines.push('        }');
  }
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
