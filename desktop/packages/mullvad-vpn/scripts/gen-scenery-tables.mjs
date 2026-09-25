#!/usr/bin/env node
// Writes the Android and iOS scenery tables from scenery.json, the one table of
// layers, countries and phase rows. Each platform keeps its resolver in its own
// language; only the data block between the GENERATED markers comes from here,
// so a country or a phase row can never exist on one platform only. The desktop
// renderer and the browser extension read scenery.json directly.
//
//   node scripts/gen-scenery-tables.mjs          rewrite the blocks
//   node scripts/gen-scenery-tables.mjs --check  exit 1 when a block is stale
//
// process-scenery.sh runs it; test/unit/scenery-assets.spec.ts fails on a stale
// block.
import { readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

const PACKAGE = 'desktop/packages/mullvad-vpn';
const MANIFEST = `${PACKAGE}/assets/images/scenery/scenery.json`;
const KOTLIN =
  'android/lib/feature/home/impl/src/main/kotlin/com/warrenbrowse/vpn/feature/home/impl/connect/ConnectionPhase.kt';
const SWIFT = 'ios/WarrenVPN/View controllers/Tunnel/MapViewController.swift';

const BEGIN = 'BEGIN GENERATED scenery table';
const END = 'END GENERATED scenery table';
const PHASES = ['exposed', 'connecting', 'protected', 'interrupted', 'blocked'];

function layerOf(manifest, role) {
  const layer = manifest.layers.find((l) => l.role === role);
  if (!layer) throw new Error(`scenery.json has no ${role} layer`);
  return layer;
}

/** Every key a country is looked up by: its ISO code and its English
 * relay-list name, both lower-case, as the desktop and Android resolvers do. */
function countryKeys(manifest) {
  return manifest.layers
    .filter((l) => l.role === 'country')
    .flatMap((l) => [
      [l.country.toLowerCase(), l],
      [l.countryName.toLowerCase(), l],
    ]);
}

function phaseRow(manifest, phase) {
  const row = manifest.phases[phase];
  if (!row) throw new Error(`scenery.json has no ${phase} phase`);
  return row;
}

const capitalize = (s) => s[0].toUpperCase() + s.slice(1);

export function renderKotlin(manifest) {
  const drawable = (layer) => `R.drawable.scenery_${layer.slug}`;
  const lines = [
    `// ${BEGIN}: scripts/gen-scenery-tables.mjs from scenery.json, do not edit.`,
    '',
    '/** Which landscape a phase shows (the exit country, or the plain), and the other two layers. */',
    'internal data class SceneryRow(',
    '    val countryLandscape: Boolean,',
    '    val showBula: Boolean,',
    '    val blurred: Boolean,',
    ')',
    '',
    'internal object SceneryTable {',
    `    @DrawableRes val plain: Int = ${drawable(layerOf(manifest, 'plain'))}`,
    `    @DrawableRes val burrow: Int = ${drawable(layerOf(manifest, 'burrow'))}`,
    `    @DrawableRes val bula: Int = ${drawable(layerOf(manifest, 'bula'))}`,
    '',
    '    /** Keyed by the lower-case ISO code and the lower-case English relay-list name. */',
    '    val countries: Map<String, Int> =',
    '        mapOf(',
    ...countryKeys(manifest).map(([key, layer]) => `            "${key}" to ${drawable(layer)},`),
    '        )',
    '',
    '    fun row(phase: ConnectionPhase): SceneryRow =',
    '        when (phase) {',
    ...PHASES.map((phase) => {
      const row = phaseRow(manifest, phase);
      return (
        `            ConnectionPhase.${capitalize(phase)} ->\n` +
        `                SceneryRow(countryLandscape = ${row.landscape === 'country'}, showBula = ${row.bula}, blurred = ${row.blurred})`
      );
    }),
    '        }',
    '}',
    '',
    `// ${END}`,
  ];
  return lines.join('\n');
}

export function renderSwift(manifest) {
  const lines = [
    `    // ${BEGIN}: scripts/gen-scenery-tables.mjs from scenery.json, do not edit.`,
    '    // Keyed by the lower-case ISO code and the lower-case English relay-list name.',
    '    private static let countryImages: [String: String] = [',
    ...countryKeys(manifest).map(([key, layer]) => `        "${key}": "${layer.iosImageset}",`),
    '    ]',
    `    private static let plainImageName = "${layerOf(manifest, 'plain').iosImageset}"`,
    `    private static let burrowImageName = "${layerOf(manifest, 'burrow').iosImageset}"`,
    `    private static let bulaImageName = "${layerOf(manifest, 'bula').iosImageset}"`,
    '',
    '    // Which landscape a phase shows (the exit country, or the plain), and the other two layers.',
    '    private static func sceneryRow(',
    '        _ phase: ConnectionPhase',
    '    ) -> (countryLandscape: Bool, showsBula: Bool, blurred: Bool) {',
    '        switch phase {',
    ...PHASES.map((phase) => {
      const row = phaseRow(manifest, phase);
      return (
        `        case .${phase}:\n` +
        `            return (countryLandscape: ${row.landscape === 'country'}, showsBula: ${row.bula}, blurred: ${row.blurred})`
      );
    }),
    '        }',
    '    }',
    `    // ${END}`,
  ];
  return lines.join('\n');
}

/** Replaces the marked block of `source`, markers included, with `block`. */
export function spliceGenerated(source, block, file) {
  const begin = source.indexOf(BEGIN);
  const end = source.indexOf(END);
  if (begin < 0 || end < begin) throw new Error(`${file} has no ${BEGIN} ... ${END} block`);
  const lineStart = source.lastIndexOf('\n', begin) + 1;
  const lineEnd = source.indexOf('\n', end);
  return source.slice(0, lineStart) + block + source.slice(lineEnd < 0 ? source.length : lineEnd);
}

/** What each generated file must hold, for the spec and for --check. */
export function generatedTables(repo) {
  const manifest = JSON.parse(readFileSync(path.join(repo, MANIFEST), 'utf8'));
  return [
    [KOTLIN, renderKotlin(manifest)],
    [SWIFT, renderSwift(manifest)],
  ].map(([relative, block]) => {
    const file = path.join(repo, relative);
    return { file, expected: spliceGenerated(readFileSync(file, 'utf8'), block, relative) };
  });
}

if (process.argv[1] && fileURLToPath(import.meta.url) === path.resolve(process.argv[1])) {
  const repo = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../../..');
  const check = process.argv.includes('--check');
  let stale = 0;
  for (const { file, expected } of generatedTables(repo)) {
    if (readFileSync(file, 'utf8') === expected) continue;
    stale += 1;
    if (check) console.error(`stale: ${path.relative(repo, file)}`);
    else writeFileSync(file, expected);
  }
  if (check && stale > 0) process.exit(1);
  console.log(check ? 'scenery tables are current' : `scenery tables written (${stale} updated)`);
}
