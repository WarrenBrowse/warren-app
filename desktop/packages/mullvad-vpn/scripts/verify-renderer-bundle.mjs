// Fails when the built renderer bundle references a Node global it cannot have.
//
// The renderer runs sandboxed, with no Node. Anything that drags the `electron`
// package (or another Node-only module) into its bundle makes Vite emit a
// `__dirname` reference, and the bundle then throws
// `Uncaught ReferenceError: __dirname is not defined` on load. The React app
// never mounts, so the window paints nothing and clicking the tray icon looks
// like a dead app. That shipped as 1.1.5 and cost every desktop user the whole
// interface.
//
// Type-checking and linting both pass on such a bundle, which is why this is a
// check on the ARTIFACT and not on the sources. The ESLint rule stops the known
// import; this stops whatever finds the next route in.
//
// `preload.cjs` is excluded on purpose: it is a separate bundle that runs WITH
// Node, and it is what exposes `window.ipc` in the first place.

import { readFileSync } from 'node:fs';
import { glob } from 'node:fs/promises';
import { basename } from 'node:path';

const NODE_ONLY_GLOBALS = ['__dirname', '__filename'];

const offenders = [];
for await (const file of glob('build/assets/*.js')) {
  const source = readFileSync(file, 'utf8');
  for (const global of NODE_ONLY_GLOBALS) {
    // Word boundary, so a string like "my__dirname" is not a false positive.
    if (new RegExp(`\\b${global}\\b`).test(source)) {
      offenders.push(`${basename(file)} references ${global}`);
    }
  }
}

if (offenders.length > 0) {
  console.error('The renderer bundle references Node globals it cannot have:');
  for (const offender of offenders) {
    console.error(`  - ${offender}`);
  }
  console.error(
    '\nThe sandboxed renderer has no Node, so this bundle throws on load and the\n' +
      'window never paints. Something imported a Node-only module (usually the\n' +
      '`electron` package, via lib/ipc-event-channel). Use window.ipc instead.',
  );
  process.exit(1);
}

console.log('renderer bundle carries no Node-only globals');
