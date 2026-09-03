// Ports the desktop round-flag set (HatScripts circle-flags, MIT, see
// desktop/packages/mullvad-vpn/assets/images/flags/LICENSE.md) to Android
// vector drawables, one `flag_<code>.xml` per SVG, plus the `FlagAssets`
// lookup that maps an ISO 3166-1 alpha-2 code onto its drawable. The desktop
// set is the source of truth: `test/unit/flag-assets.spec.ts` regenerates
// everything in memory and fails when the Android tree is stale against it.
//
// The SVGs are all authored the same way: a 512 x 512 canvas, a circular mask,
// and a handful of `path`, `circle`, `rect` and `ellipse` elements with flat
// fills, optionally grouped under a shared fill. VectorDrawable has no mask and
// draws paths only, so the mask becomes a clip-path and every shape becomes
// path data.

import { readdirSync, readFileSync } from 'node:fs';
import path from 'node:path';

export const FLAGS_DIR = 'desktop/packages/mullvad-vpn/assets/images/flags';
export const DRAWABLE_DIR = 'android/lib/ui/resource/src/main/res/drawable';
export const KOTLIN_PATH =
  'android/lib/ui/resource/src/main/kotlin/com/warrenbrowse/vpn/lib/ui/resource/FlagAssets.kt';

const SVG_HEAD =
  '<svg xmlns="http://www.w3.org/2000/svg" width="512" height="512" viewBox="0 0 512 512">' +
  '<mask id="a"><circle cx="256" cy="256" r="256" fill="#fff"/></mask><g mask="url(#a)">';
const SVG_TAIL = '</g></svg>';

// The circular mask every flag is cut with, as path data (two half-circle arcs).
const CLIP = 'M256 0A256 256 0 1 1 256 512A256 256 0 1 1 256 0Z';

/** The lowercase alpha-2 codes of the desktop set, sorted. */
export function flagCodes(repoRoot) {
  return readdirSync(path.join(repoRoot, FLAGS_DIR))
    .filter((name) => name.endsWith('.svg'))
    .map((name) => name.slice(0, -4))
    .sort();
}

/** `#eee` and `#0052b4` to the `#RRGGBB` VectorDrawable expects. */
export function expandHex(fill) {
  const m = /^#([0-9a-fA-F]{3}|[0-9a-fA-F]{6})$/.exec(fill);
  if (!m) throw new Error(`unsupported fill ${fill}`);
  const hex = m[1].length === 3 ? [...m[1]].map((c) => c + c).join('') : m[1];
  return `#${hex.toUpperCase()}`;
}

const attrs = (text) => Object.fromEntries([...text.matchAll(/([\w-]+)="([^"]*)"/g)].map((m) => [m[1], m[2]]));

const n = (value) => {
  const out = Number(value);
  if (Number.isNaN(out)) throw new Error(`not a number: ${value}`);
  return out;
};

// Numbers are printed the way the SVGs print them (no trailing zeros), so the
// path data stays short and stable across runs.
const f = (value) => String(Number(value.toFixed(3)));

export function circlePath(cx, cy, r) {
  return `M${f(cx - r)} ${f(cy)}a${f(r)} ${f(r)} 0 1 0 ${f(2 * r)} 0a${f(r)} ${f(r)} 0 1 0 ${f(-2 * r)} 0z`;
}

export function ellipsePath(cx, cy, rx, ry) {
  return `M${f(cx - rx)} ${f(cy)}a${f(rx)} ${f(ry)} 0 1 0 ${f(2 * rx)} 0a${f(rx)} ${f(ry)} 0 1 0 ${f(-2 * rx)} 0z`;
}

export function rectPath(x, y, w, h, rxIn, ryIn) {
  // SVG: a missing rx takes ry and vice versa; both are clamped to half the side.
  let rx = rxIn ?? ryIn ?? 0;
  let ry = ryIn ?? rxIn ?? 0;
  rx = Math.min(rx, w / 2);
  ry = Math.min(ry, h / 2);
  if (rx === 0 && ry === 0) return `M${f(x)} ${f(y)}h${f(w)}v${f(h)}h${f(-w)}z`;
  return (
    `M${f(x + rx)} ${f(y)}` +
    `h${f(w - 2 * rx)}` +
    `a${f(rx)} ${f(ry)} 0 0 1 ${f(rx)} ${f(ry)}` +
    `v${f(h - 2 * ry)}` +
    `a${f(rx)} ${f(ry)} 0 0 1 ${f(-rx)} ${f(ry)}` +
    `h${f(-(w - 2 * rx))}` +
    `a${f(rx)} ${f(ry)} 0 0 1 ${f(-rx)} ${f(-ry)}` +
    `v${f(-(h - 2 * ry))}` +
    `a${f(rx)} ${f(ry)} 0 0 1 ${f(rx)} ${f(-ry)}z`
  );
}

/** The `<path .../>` lines of one flag, fills resolved through the enclosing groups. */
export function svgToPaths(svg) {
  const text = svg.trim();
  if (!text.startsWith(SVG_HEAD) || !text.endsWith(SVG_TAIL)) {
    throw new Error('flag SVG is not in the circle-flags shape');
  }
  const body = text.slice(SVG_HEAD.length, text.length - SVG_TAIL.length);
  const out = [];
  const fills = [];
  const tags = body.matchAll(/<(\/?)([a-z]+)([^>]*?)(\/?)>/g);
  for (const [, closing, name, rawAttrs, selfClosing] of tags) {
    if (closing) {
      if (name !== 'g') throw new Error(`unexpected </${name}>`);
      fills.pop();
      continue;
    }
    const a = attrs(rawAttrs);
    if (name === 'g') {
      if (selfClosing) continue;
      fills.push(a.fill);
      continue;
    }
    // SVG paints an unfilled shape black; `fill="none"` paints nothing and
    // there are no strokes in the set.
    const fill = a.fill ?? fills.findLast((v) => v !== undefined) ?? '#000';
    if (fill === 'none') continue;
    let d;
    switch (name) {
      case 'path':
        d = a.d;
        break;
      case 'circle':
        d = circlePath(n(a.cx), n(a.cy), n(a.r));
        break;
      case 'ellipse':
        d = ellipsePath(n(a.cx), n(a.cy), n(a.rx), n(a.ry));
        break;
      case 'rect':
        d = rectPath(
          n(a.x ?? 0),
          n(a.y ?? 0),
          n(a.width),
          n(a.height),
          a.rx === undefined ? undefined : n(a.rx),
          a.ry === undefined ? undefined : n(a.ry),
        );
        break;
      default:
        throw new Error(`unsupported element <${name}>`);
    }
    if (!d) throw new Error(`<${name}> without path data`);
    out.push({ fill: expandHex(fill), d });
  }
  return out;
}

/** One Android vector drawable, byte for byte. */
export function svgToVectorDrawable(svg, code) {
  const lines = [
    `<!-- GENERATED by scripts/flags/gen.mjs from ${FLAGS_DIR}/${code}.svg (circle-flags, MIT). Do not edit. -->`,
    '<vector xmlns:android="http://schemas.android.com/apk/res/android"',
    '    android:width="22dp" android:height="22dp"',
    '    android:viewportWidth="512" android:viewportHeight="512">',
    '    <group>',
    `        <clip-path android:pathData="${CLIP}" />`,
  ];
  for (const { fill, d } of svgToPaths(svg)) {
    lines.push(`        <path android:fillColor="${fill}" android:pathData="${d}" />`);
  }
  lines.push('    </group>');
  lines.push('</vector>');
  return `${lines.join('\n')}\n`;
}

export const drawableName = (code) => `flag_${code}`;

/** The Kotlin lookup, byte for byte. */
export function renderFlagAssets(codes) {
  const lines = [
    '// GENERATED FILE, DO NOT EDIT.',
    '//',
    `// scripts/flags/gen.mjs writes this lookup and the flag_*.xml drawables from the`,
    `// desktop flag set in ${FLAGS_DIR} (circle-flags, MIT). The desktop is the`,
    '// source of truth; test/unit/flag-assets.spec.ts fails when either is stale.',
    'package com.warrenbrowse.vpn.lib.ui.resource',
    '',
    '/**',
    ' * The round country flags shared with the desktop client, by ISO 3166-1',
    ' * alpha-2 code. Both clients draw the same artwork, so the flag in the',
    ' * connection card is the same flag on every platform.',
    ' */',
    'object FlagAssets {',
    '    /** The drawable for [countryCode] in any case, or null when the set has no flag for it. */',
    '    fun drawableFor(countryCode: String): Int? =',
    '        when (countryCode.lowercase()) {',
  ];
  for (const code of codes) lines.push(`            "${code}" -> R.drawable.${drawableName(code)}`);
  lines.push('            else -> null');
  lines.push('        }');
  lines.push('}');
  return `${lines.join('\n')}\n`;
}

/** Every generated file as `{ relativePath: text }`. */
export function buildFlagOutputs(repoRoot) {
  const codes = flagCodes(repoRoot);
  const outputs = {};
  for (const code of codes) {
    const svg = readFileSync(path.join(repoRoot, FLAGS_DIR, `${code}.svg`), 'utf8');
    outputs[`${DRAWABLE_DIR}/${drawableName(code)}.xml`] = svgToVectorDrawable(svg, code);
  }
  outputs[KOTLIN_PATH] = renderFlagAssets(codes);
  return outputs;
}
