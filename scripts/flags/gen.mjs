#!/usr/bin/env node
// Regenerates the Android flag drawables and the FlagAssets lookup from the
// desktop flag set. `--check` exits non-zero when any output is stale or when
// a drawable no longer has a desktop source.
import { existsSync, mkdirSync, readdirSync, readFileSync, unlinkSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { buildFlagOutputs, DRAWABLE_DIR } from './lib.mjs';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const outputs = buildFlagOutputs(repoRoot);

// A drawable whose SVG left the desktop set is stale too.
const orphans = readdirSync(path.join(repoRoot, DRAWABLE_DIR))
  .filter((name) => /^flag_[a-z]{2}\.xml$/.test(name))
  .map((name) => `${DRAWABLE_DIR}/${name}`)
  .filter((relative) => !(relative in outputs));

if (process.argv.includes('--check')) {
  let stale = false;
  for (const [relative, expected] of Object.entries(outputs)) {
    const file = path.join(repoRoot, relative);
    if (!existsSync(file) || readFileSync(file, 'utf8') !== expected) {
      console.error(`stale: ${relative}`);
      stale = true;
    }
  }
  for (const relative of orphans) {
    console.error(`orphan: ${relative}`);
    stale = true;
  }
  process.exit(stale ? 1 : 0);
}

for (const [relative, text] of Object.entries(outputs)) {
  const file = path.join(repoRoot, relative);
  mkdirSync(path.dirname(file), { recursive: true });
  writeFileSync(file, text);
}
for (const relative of orphans) unlinkSync(path.join(repoRoot, relative));
console.log(`wrote ${Object.keys(outputs).length} files, removed ${orphans.length} orphans`);
