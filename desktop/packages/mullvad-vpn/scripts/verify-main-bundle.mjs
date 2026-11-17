// Fails when the built main-process bundle still carries the development-only
// extension installer.
//
// `electron-devtools-installer` downloads Chrome extensions from Google and
// unpacks them. It is only ever called under NODE_ENV=development, but a
// dynamic import the bundler cannot prove dead still ships as its own chunk in
// every release build. This checks the artifact, where that shows.

import { readFileSync } from 'node:fs';
import { glob } from 'node:fs/promises';
import { basename } from 'node:path';

const DEVELOPMENT_ONLY_MARKERS = ['clients2.google.com', 'electron-devtools-installer'];

const offenders = [];
for await (const file of glob('build/*.js')) {
  const source = readFileSync(file, 'utf8');
  for (const marker of DEVELOPMENT_ONLY_MARKERS) {
    if (source.includes(marker)) {
      offenders.push(`${basename(file)} contains ${marker}`);
    }
  }
}

if (offenders.length > 0) {
  console.error('The main-process bundle carries development-only code:');
  for (const offender of offenders) {
    console.error(`  - ${offender}`);
  }
  process.exit(1);
}

console.log('main-process bundle carries no development-only extension installer');
