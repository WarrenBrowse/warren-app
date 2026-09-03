import fs from 'fs';
import path from 'path';
import { afterEach, describe, expect, it } from 'vitest';

import { importForProductEnv } from './client-rules';

type TrayIconModule = typeof import('../../src/main/tray-icon');
type TrayIconControllerModule = typeof import('../../src/main/tray-icon-controller');

// The resolver anchors on `import.meta.dirname`, which points at src/main in a
// source tree and at build/ in a packaged app. The assets themselves only ever
// live in the source tree, so the suite checks the segment the resolver appends
// against the real asset directory.
const ASSETS_DIR = path.resolve(__dirname, '../../assets/images/menubar-icons');

const BADGED_DIR = 'beta';
const PLATFORMS = ['darwin', 'linux', 'win32'] as const;
const FRAMES = Array.from({ length: 10 }, (_, index) => index + 1);

const realPlatform = process.platform;

function switchPlatform(platform: string) {
  Object.defineProperty(process, 'platform', { value: platform, configurable: true });
}

async function trayIconModule(productEnv: string) {
  return importForProductEnv<TrayIconModule>(productEnv, '../../src/main/tray-icon');
}

async function trayIconController() {
  return importForProductEnv<TrayIconControllerModule>(
    'prod',
    '../../src/main/tray-icon-controller',
  );
}

/** The path the resolver appends to its base, as a `/`-separated string. */
function assetPath(icon: InstanceType<TrayIconModule['TrayIcon']>): string {
  const filePath = icon.filePath;
  if (filePath === null) {
    throw new Error('the resolver returned no path for a named icon');
  }
  return path.relative(icon.basePath, filePath).split(path.sep).join('/');
}

/**
 * Every icon name the controller can ask for on `platform`: the whole
 * monochrome x notification matrix, driven by the production matrix itself
 * rather than a copy of it, plus the Linux startup placeholder.
 */
async function iconNamesFor(platform: string): Promise<string[]> {
  const { trayIconSuffix } = await trayIconController();
  const names: string[] = [];
  for (const monochromatic of [false, true]) {
    for (const notification of [false, true]) {
      // Only Windows reads the system theme; the other platforms ignore it.
      const themes = platform === 'win32' ? [true, false, undefined] : [undefined];
      for (const systemUsesLightTheme of themes) {
        const suffix = trayIconSuffix(platform, monochromatic, notification, systemUsesLightTheme);
        names.push(...FRAMES.map((frame) => `lock-${frame}${suffix}`));
      }
    }
  }
  if (platform === 'linux') {
    names.push('lock-placeholder');
  }
  return [...new Set(names)];
}

function extensionFor(platform: string): string {
  return platform === 'win32' ? 'ico' : 'png';
}

/** Escapes a literal, then expands the `*` and `**` of an asarUnpack glob. */
function globToRegExp(pattern: string): RegExp {
  const escaped = pattern.replace(/[.+^${}()|[\]\\]/g, '\\$&');
  const body = escaped
    .split('/**/')
    .map((segment) =>
      segment
        .split('**')
        .map((part) => part.split('*').join('[^/]*'))
        .join('.*'),
    )
    .join('/(?:.*/)?');
  return new RegExp(`^${body}$`);
}

describe('the tray icon of a non-prod build', () => {
  afterEach(() => {
    switchPlatform(realPlatform);
  });

  it('resolves the unbadged tree in a prod build, on every platform', async () => {
    const { TrayIcon } = await trayIconModule('prod');
    for (const platform of PLATFORMS) {
      switchPlatform(platform);
      expect(assetPath(new TrayIcon('lock-1')), platform).toBe(
        `${platform}/lock-1.${extensionFor(platform)}`,
      );
    }
  });

  it('resolves the badged tree for every icon the controller can ask for', async () => {
    // Staging is badged too. The packaging identity already hands staging the
    // beta app icon (iconSuffix in tasks/distribution.cjs) because what matters
    // is being tellable from prod, and there is no third artwork.
    for (const productEnv of ['beta', 'staging']) {
      const { TrayIcon } = await trayIconModule(productEnv);
      for (const platform of PLATFORMS) {
        switchPlatform(platform);
        for (const name of await iconNamesFor(platform)) {
          expect(assetPath(new TrayIcon(name)), `${productEnv} ${platform} ${name}`).toBe(
            `${platform}/${BADGED_DIR}/${name}.${extensionFor(platform)}`,
          );
        }
      }
    }
  });

  it('resolves only paths that exist on disk', async () => {
    const { TrayIcon } = await trayIconModule('beta');
    for (const platform of PLATFORMS) {
      switchPlatform(platform);
      for (const name of await iconNamesFor(platform)) {
        const relative = assetPath(new TrayIcon(name));
        expect(fs.existsSync(path.join(ASSETS_DIR, relative)), relative).toBe(true);
        if (platform === 'darwin') {
          // Electron picks the retina file up off disk by name, so it is never
          // resolved but it has to be there.
          const retina = relative.replace(/\.png$/, '@2x.png');
          expect(fs.existsSync(path.join(ASSETS_DIR, retina)), retina).toBe(true);
        }
      }
    }
  });

  it('badges every asset, with bytes of its own', () => {
    // Identical file names in a sibling directory keep the controller's suffix
    // matrix untouched, which also means a stale or half-regenerated badged
    // tree is invisible to a path comparison. Compare the bytes.
    for (const platform of PLATFORMS) {
      const prodDir = path.join(ASSETS_DIR, platform);
      const badgedDir = path.join(prodDir, BADGED_DIR);
      const prodFiles = fs
        .readdirSync(prodDir, { withFileTypes: true })
        .filter((entry) => entry.isFile())
        .map((entry) => entry.name)
        .sort();

      expect(prodFiles.length, platform).toBeGreaterThan(0);
      expect(
        fs
          .readdirSync(badgedDir, { withFileTypes: true })
          .filter((entry) => entry.isFile())
          .map((entry) => entry.name)
          .sort(),
        platform,
      ).toEqual(prodFiles);

      for (const name of prodFiles) {
        const badged = fs.readFileSync(path.join(badgedDir, name));
        const prod = fs.readFileSync(path.join(prodDir, name));
        expect(badged.equals(prod), `${platform}/${name}`).toBe(false);
      }
    }
  });

  it('is unpacked from the Windows asar archive, badged tree included', async () => {
    // Windows cannot load a tray .ico from inside the asar. A glob that stops
    // at the prod directory leaves the badged icons packed, and the beta tray
    // is then empty at runtime, with no error, in packaged builds only.
    const { WINDOWS_ASAR_UNPACK } = await import('../../tasks/distribution.cjs');
    const globs = WINDOWS_ASAR_UNPACK.map(globToRegExp);

    switchPlatform('win32');
    for (const productEnv of ['prod', 'beta', 'staging']) {
      const { TrayIcon } = await trayIconModule(productEnv);
      for (const name of await iconNamesFor('win32')) {
        const packaged = `build/assets/images/menubar-icons/${assetPath(new TrayIcon(name))}`;
        expect(
          globs.some((glob) => glob.test(packaged)),
          `${productEnv} ${packaged}`,
        ).toBe(true);
      }
    }
  });
});
