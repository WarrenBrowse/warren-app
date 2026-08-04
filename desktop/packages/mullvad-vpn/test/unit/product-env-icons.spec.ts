import fs from 'fs';
import { afterEach, describe, expect, it, vi } from 'vitest';

// The packaging config reads WARREN_PRODUCT_ENV once, at module load, so each
// environment needs a fresh module instance.
async function iconsFor(productEnv: string) {
  vi.resetModules();
  process.env.WARREN_PRODUCT_ENV = productEnv;
  const distribution = await import('../../tasks/distribution.cjs');
  const config = distribution.newConfig();
  return {
    macos: config.mac.icon,
    linux: config.linux.icon,
    windows: config.win.icon,
    installerSidebar: config.nsis.installerSidebar,
  };
}

describe('per-environment app icons', () => {
  afterEach(() => {
    delete process.env.WARREN_PRODUCT_ENV;
  });

  // Loading the packaging config three times over is slow enough to blow the
  // default 5s budget.
  const timeoutMs = 60_000;

  it(
    'points every platform at an icon that exists on disk',
    async () => {
      for (const productEnv of ['prod', 'beta', 'staging']) {
        const icons = await iconsFor(productEnv);
        for (const [platform, iconPath] of Object.entries(icons)) {
          expect(fs.existsSync(iconPath), `${productEnv} ${platform}: ${iconPath}`).toBe(true);
        }
      }
    },
    timeoutMs,
  );

  // A non-prod install has to be tellable from a prod one on the desktop, in
  // the dock and in the installer, so it may never reuse a prod icon asset.
  it(
    'gives a non-prod build its own icon on every platform',
    async () => {
      const prod = await iconsFor('prod');
      for (const productEnv of ['beta', 'staging']) {
        const icons = await iconsFor(productEnv);
        for (const platform of Object.keys(prod) as (keyof typeof prod)[]) {
          expect(icons[platform], `${productEnv} ${platform}`).not.toBe(prod[platform]);
        }
      }
    },
    timeoutMs,
  );
});
