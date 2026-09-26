#!/usr/bin/env node
// Writes the Android and iOS scenery tables and phase colour tokens from
// scenery.json, the one table of layers, countries and phase rows. Each platform keeps its resolver in its own
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
const SWIFT_PHASE =
  'ios/WarrenVPN/View controllers/Tunnel/ConnectionView/ConnectionViewViewModel.swift';

const SCENERY_TABLE = 'scenery table';
const PHASE_COLOURS = 'phase colour table';
const begin = (name) => `BEGIN GENERATED ${name}`;
const end = (name) => `END GENERATED ${name}`;
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
    `// ${begin(SCENERY_TABLE)}: scripts/gen-scenery-tables.mjs from scenery.json, do not edit.`,
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
    `// ${end(SCENERY_TABLE)}`,
  ];
  return lines.join('\n');
}

export function renderSwift(manifest) {
  const lines = [
    `    // ${begin(SCENERY_TABLE)}: scripts/gen-scenery-tables.mjs from scenery.json, do not edit.`,
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
    `    // ${end(SCENERY_TABLE)}`,
  ];
  return lines.join('\n');
}

/** Every colour token the phase rows use, in a stable order. */
function tones(manifest) {
  const all = PHASES.flatMap((phase) => [
    phaseRow(manifest, phase).accent,
    phaseRow(manifest, phase).title,
  ]);
  return [...new Set(all)].sort();
}

export function renderKotlinPhaseColours(manifest) {
  const tone = (name) => `SceneryTone.${capitalize(name)}`;
  const byPhase = (field) =>
    PHASES.map(
      (phase) =>
        `        ConnectionPhase.${capitalize(phase)} -> ${tone(phaseRow(manifest, phase)[field])}`,
    );
  return [
    `// ${begin(PHASE_COLOURS)}: scripts/gen-scenery-tables.mjs from scenery.json, do not edit.`,
    '',
    '/** The colour tokens scenery.json paints a phase with; the theme maps each one once. */',
    'internal enum class SceneryTone {',
    ...tones(manifest).map((name) => `    ${capitalize(name)},`),
    '}',
    '',
    '/** The saturated accent a phase fills its eye well, rail and buttons with. */',
    'internal fun ConnectionPhase.accentTone(): SceneryTone =',
    '    when (this) {',
    ...byPhase('accent'),
    '    }',
    '',
    '/** The lifted tint a phase writes its title with. */',
    'internal fun ConnectionPhase.titleTone(): SceneryTone =',
    '    when (this) {',
    ...byPhase('title'),
    '    }',
    '',
    `// ${end(PHASE_COLOURS)}`,
  ].join('\n');
}

export function renderSwiftPhaseColours(manifest) {
  const byPhase = (field) =>
    PHASES.map((phase) => `        case .${phase}: .${phaseRow(manifest, phase)[field]}`);
  return [
    `// ${begin(PHASE_COLOURS)}: scripts/gen-scenery-tables.mjs from scenery.json, do not edit.`,
    '/// The colour tokens scenery.json paints a phase with; `SceneryTone.color` maps each one once.',
    'enum SceneryTone {',
    ...tones(manifest).map((name) => `    case ${name}`),
    '}',
    '',
    'extension ConnectionPhase {',
    '    /// The saturated accent a phase fills its eye, rails and buttons with.',
    '    var accentTone: SceneryTone {',
    '        switch self {',
    ...byPhase('accent'),
    '        }',
    '    }',
    '',
    '    /// The lifted tint a phase writes its title with.',
    '    var titleTone: SceneryTone {',
    '        switch self {',
    ...byPhase('title'),
    '        }',
    '    }',
    '}',
    `// ${end(PHASE_COLOURS)}`,
  ].join('\n');
}

/** Replaces the block `name` of `source`, markers included, with `block`. */
export function spliceGenerated(source, block, file, name) {
  const from = source.indexOf(begin(name));
  const to = source.indexOf(end(name));
  if (from < 0 || to < from)
    throw new Error(`${file} has no ${begin(name)} ... ${end(name)} block`);
  const lineStart = source.lastIndexOf('\n', from) + 1;
  const lineEnd = source.indexOf('\n', to);
  return source.slice(0, lineStart) + block + source.slice(lineEnd < 0 ? source.length : lineEnd);
}

/** What each generated file must hold, for the spec and for --check. */
export function generatedTables(repo) {
  const manifest = JSON.parse(readFileSync(path.join(repo, MANIFEST), 'utf8'));
  const files = [
    [
      KOTLIN,
      [
        [SCENERY_TABLE, renderKotlin(manifest)],
        [PHASE_COLOURS, renderKotlinPhaseColours(manifest)],
      ],
    ],
    [SWIFT, [[SCENERY_TABLE, renderSwift(manifest)]]],
    [SWIFT_PHASE, [[PHASE_COLOURS, renderSwiftPhaseColours(manifest)]]],
  ];
  return files.map(([relative, blocks]) => {
    const file = path.join(repo, relative);
    const expected = blocks.reduce(
      (source, [name, block]) => spliceGenerated(source, block, relative, name),
      readFileSync(file, 'utf8'),
    );
    return { file, expected };
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
