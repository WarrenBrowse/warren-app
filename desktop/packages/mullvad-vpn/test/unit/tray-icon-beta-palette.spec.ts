import fs from 'fs';
import path from 'path';
import { afterEach, describe, expect, it } from 'vitest';

import { importForProductEnv } from './client-rules';
import { dominantColours } from './png-pixels';

type TrayIconModule = typeof import('../../src/main/tray-icon');
type TrayIconControllerModule = typeof import('../../src/main/tray-icon-controller');

// The resolver anchors on `import.meta.dirname`, which points at src/main in a
// source tree and at build/ in a packaged app. The assets themselves only ever
// live in the source tree, so the suite checks the segment the resolver appends
// against the real asset directory.
const ASSETS_DIR = path.resolve(__dirname, '../../assets/images/menubar-icons');

const SVG_DIR = path.join(ASSETS_DIR, 'svg');
const PALETTE_FILE = path.resolve(__dirname, '../../../../../graphics/menubar-beta-palette.txt');

const BADGED_DIR = 'beta';
const PLATFORMS = ['darwin', 'linux', 'win32'] as const;
// The two that ship a plain PNG, whose pixels can be read back.
const PIXEL_PLATFORMS = ['darwin', 'linux'] as const;
const FRAMES = Array.from({ length: 10 }, (_, index) => index + 1);

/** The production accent to beta accent table the generator applies. */
function palette(): Map<string, string> {
  const rows = fs
    .readFileSync(PALETTE_FILE, 'utf8')
    .split('\n')
    // A data row is `#RRGGBB` and its replacement. Every comment line starts
    // with a hash and a space, so the two can never be confused.
    .map((line) => /^(#[0-9A-Fa-f]{6})\s+(#[0-9A-Fa-f]{6})\s*$/.exec(line))
    .filter((match): match is RegExpExecArray => match !== null);
  expect(rows.length, PALETTE_FILE).toBeGreaterThan(0);
  return new Map(rows.map((row) => [row[1].toUpperCase(), row[2].toUpperCase()]));
}

/** Every colour a set of SVG sources paints with. */
function svgColours(files: string[]): Set<string> {
  const found = new Set<string>();
  for (const file of files) {
    for (const colour of fs.readFileSync(file, 'utf8').match(/#[0-9A-Fa-f]{6}/g) ?? []) {
      found.add(colour.toUpperCase());
    }
  }
  return found;
}

function lockSources(): string[] {
  return fs
    .readdirSync(SVG_DIR)
    .filter((name) => name.startsWith('lock-') && !name.startsWith('lock-placeholder'))
    .map((name) => path.join(SVG_DIR, name));
}

// A monochrome variant is a single-tint alpha mask: macOS tints its Template
// images itself, and the Windows and Linux ones are drawn in flat black or
// white. There is no colour in one to move.
const isMonochrome = (name: string) => /Template|_white|_black/.test(name);
// The startup placeholder is drawn in neutral greys and shows no tunnel state,
// so it carries no accent either.
const isPlaceholder = (name: string) => name.startsWith('lock-placeholder');
const isColoured = (name: string) => !isMonochrome(name) && !isPlaceholder(name);

function filesIn(dir: string): string[] {
  return fs
    .readdirSync(dir, { withFileTypes: true })
    .filter((entry) => entry.isFile())
    .map((entry) => entry.name)
    .sort();
}

const sorted = (colours: Iterable<string>) => [...colours].sort();

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

  it('ships the same file names as the production tree', () => {
    // Identical file names in a sibling directory keep the controller's suffix
    // matrix untouched, which also means a missing file stays invisible to a
    // path comparison until the tray is empty at runtime.
    for (const platform of PLATFORMS) {
      const prodDir = path.join(ASSETS_DIR, platform);
      const prodFiles = filesIn(prodDir);

      expect(prodFiles.length, platform).toBeGreaterThan(0);
      expect(filesIn(path.join(prodDir, BADGED_DIR)), platform).toEqual(prodFiles);
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

describe('the tray lock of a non-prod build', () => {
  it('has a beta accent for every colour the lock frames are drawn in', () => {
    // A frame restyled or added in production must not reach a beta build
    // still wearing the production colours, so the table is checked against
    // the sources rather than the other way round. The notification dot is
    // deliberately absent from it: it means "attention" in both builds, and it
    // is composited over the lock rather than being part of it.
    const table = palette();

    for (const colour of svgColours(lockSources())) {
      expect(table.has(colour), `${colour} is painted by a lock frame`).toBe(true);
    }
  });

  it('is drawn in the production accents in a production build', () => {
    const table = palette();
    const notificationDot = svgColours([path.join(SVG_DIR, 'notification.svg')]);

    for (const platform of PIXEL_PLATFORMS) {
      const dir = path.join(ASSETS_DIR, platform);
      for (const name of filesIn(dir).filter(isColoured)) {
        const colours = dominantColours(fs.readFileSync(path.join(dir, name)));

        expect(colours.size, `${platform}/${name}`).toBeGreaterThan(0);
        for (const colour of colours) {
          expect(
            table.has(colour) || notificationDot.has(colour),
            `${platform}/${name} is drawn in ${colour}`,
          ).toBe(true);
        }
      }
    }
  });

  it('is the production lock recoloured, with no production accent left', () => {
    const table = palette();
    const productionAccents = new Set(table.keys());

    for (const platform of PIXEL_PLATFORMS) {
      const prodDir = path.join(ASSETS_DIR, platform);
      for (const name of filesIn(prodDir).filter(isColoured)) {
        const production = dominantColours(fs.readFileSync(path.join(prodDir, name)));
        const beta = dominantColours(fs.readFileSync(path.join(prodDir, BADGED_DIR, name)));
        // A colour the table does not name stays where it is, which is how the
        // notification dot keeps its amber in both trees.
        const expected = new Set([...production].map((colour) => table.get(colour) ?? colour));

        expect(sorted(beta), `${platform}/${BADGED_DIR}/${name}`).toEqual(sorted(expected));
        expect(
          sorted([...beta].filter((colour) => productionAccents.has(colour))),
          `${platform}/${BADGED_DIR}/${name}`,
        ).toEqual([]);
      }
    }
  });

  it('recolours the Windows icons too, whose pixels are not read here', () => {
    // An .ico packs three sizes at three colour depths each, so the palette is
    // asserted on the two platforms that ship a plain PNG and Windows is held
    // to the weaker claim that its bytes moved at all.
    const prodDir = path.join(ASSETS_DIR, 'win32');

    for (const name of filesIn(prodDir).filter(isColoured)) {
      const production = fs.readFileSync(path.join(prodDir, name));
      const beta = fs.readFileSync(path.join(prodDir, BADGED_DIR, name));

      expect(beta.equals(production), `win32/${name}`).toBe(false);
    }
  });

  it('leaves the monochrome variants identical, they carry no colour to move', () => {
    // Asserted rather than left to chance. A template image is an alpha mask
    // under a single tint, so a build whose tray icon is set to monochrome
    // cannot be told from production by its colour, and the only lever left
    // would be changing the silhouette.
    for (const platform of PLATFORMS) {
      const prodDir = path.join(ASSETS_DIR, platform);
      for (const name of filesIn(prodDir).filter((file) => !isColoured(file))) {
        const production = fs.readFileSync(path.join(prodDir, name));
        const beta = fs.readFileSync(path.join(prodDir, BADGED_DIR, name));

        expect(beta.equals(production), `${platform}/${name}`).toBe(true);
      }
    }
  });
});
