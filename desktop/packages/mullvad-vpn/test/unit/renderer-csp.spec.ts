import fs from 'fs';
import path from 'path';
import { describe, expect, it } from 'vitest';

// The renderer loads one local page and never needs the network, so its
// policy denies everything it does not use. Compiling WebAssembly was only
// ever needed by @polkadot's SS58 check, which the app now does itself.
const packageRoot = path.resolve(import.meta.dirname, '../..');

function contentSecurityPolicy(): Map<string, string[]> {
  const html = fs.readFileSync(path.join(packageRoot, 'index.html'), 'utf8');
  const match = /http-equiv="Content-Security-Policy"\s+content="([^"]*)"/.exec(html);
  expect(match, 'index.html declares a Content-Security-Policy').not.toBeNull();
  const directives = new Map<string, string[]>();
  for (const directive of match![1].split(';')) {
    const [name, ...sources] = directive.trim().split(/\s+/);
    if (name) {
      directives.set(name, sources);
    }
  }
  return directives;
}

function sourceFiles(dir: string): string[] {
  return fs.readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      return sourceFiles(full);
    }
    return /\.(ts|tsx)$/.test(entry.name) ? [full] : [];
  });
}

describe('renderer Content-Security-Policy', () => {
  it('denies by default and lets scripts come only from the app itself', () => {
    const csp = contentSecurityPolicy();
    expect(csp.get('default-src')).toEqual(["'none'"]);
    expect(csp.get('script-src')).toEqual(["'self'"]);
  });

  it('allows no plugin, base URL, form target or WebAssembly compilation', () => {
    const csp = contentSecurityPolicy();
    expect(csp.get('object-src')).toEqual(["'none'"]);
    expect(csp.get('base-uri')).toEqual(["'none'"]);
    expect(csp.get('form-action')).toEqual(["'none'"]);
    for (const sources of csp.values()) {
      expect(sources).not.toContain("'wasm-unsafe-eval'");
      expect(sources).not.toContain("'unsafe-eval'");
    }
  });

  it('is not undone by a dependency that needs WebAssembly', () => {
    const importers = sourceFiles(path.join(packageRoot, 'src')).filter((file) =>
      /from '@polkadot\//.test(fs.readFileSync(file, 'utf8')),
    );
    expect(importers.map((file) => path.relative(packageRoot, file))).toEqual([]);
  });
});
