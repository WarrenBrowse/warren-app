#!/usr/bin/env node
// Regenerates design-tokens.json and the Android DesignTokens.kt from the
// desktop token sources. Run from anywhere inside the repo; commit both
// outputs together. `--check` exits non-zero when either output is stale, for
// use in a hook or by hand before pushing.
import { readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { buildTokens, JSON_PATH, KOTLIN_PATH, renderJson, renderKotlin } from './lib.mjs';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const tokens = buildTokens(repoRoot);
const json = renderJson(tokens);
const kotlin = renderKotlin(tokens, json);

const outputs = [
  [JSON_PATH, json],
  [KOTLIN_PATH, kotlin],
];

if (process.argv.includes('--check')) {
  let stale = false;
  for (const [relative, expected] of outputs) {
    let current = '';
    try {
      current = readFileSync(path.join(repoRoot, relative), 'utf8');
    } catch {
      current = '';
    }
    if (current !== expected) {
      console.error(`stale: ${relative}`);
      stale = true;
    }
  }
  process.exit(stale ? 1 : 0);
}

for (const [relative, text] of outputs) {
  writeFileSync(path.join(repoRoot, relative), text);
  console.log(`wrote ${relative}`);
}
