import { Url, urls } from './constants/urls';

// Revisions recorded at build time by vite's `define` (see vite.config.ts):
// the app commit the bundle was built from and the engine commit it was pinned
// to. Absent outside vite (unit tests), where the links fall back to the
// repository itself.
declare const WARREN_SOURCE_COMMIT: string | undefined;
declare const WARREN_ENGINE_COMMIT: string | undefined;

const COMMIT = /^[0-9a-f]{40}$/;

function treeUrl(repository: Url, revision: string | undefined): Url {
  return revision !== undefined && COMMIT.test(revision)
    ? `${repository}tree/${revision}`
    : repository;
}

export function appSourceUrl(commit: string | undefined): Url {
  return treeUrl(urls.sourceCode, commit);
}

export function engineSourceUrl(commit: string | undefined): Url {
  return treeUrl(urls.engineSourceCode, commit);
}

export const builtAppSourceUrl = appSourceUrl(
  typeof WARREN_SOURCE_COMMIT === 'string' ? WARREN_SOURCE_COMMIT : undefined,
);

export const builtEngineSourceUrl = engineSourceUrl(
  typeof WARREN_ENGINE_COMMIT === 'string' ? WARREN_ENGINE_COMMIT : undefined,
);
