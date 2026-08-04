import { createHash } from 'crypto';
import { readdirSync, readFileSync } from 'fs';
import path from 'path';
import { describe, expect, it } from 'vitest';

// The connect screen composites a landscape, the burrow and Bula as three
// full-frame layers that only register if every client ships the SAME canvas.
// They drifted once already (Singapore stayed photoreal on desktop after the
// watercolor pass, because one platform was exported by hand), so the three
// asset trees are pinned against each other here rather than by discipline.
// Regenerate all three with scripts/process-scenery.sh, never by hand.

const REPO = path.resolve(__dirname, '../../../../..');
const DESKTOP = path.join(REPO, 'desktop/packages/mullvad-vpn/assets/images/scenery');
const ANDROID = path.join(REPO, 'android/lib/ui/resource/src/main/res/drawable-nodpi');
const IOS = path.join(REPO, 'ios/WarrenVPN/Supporting Files/Assets.xcassets');

// The canvas every layer is authored on, downscaled 3x for HiDPI.
const CANVAS = { width: 1140, height: 1706 };

// slug -> iOS imageset. Backgrounds are opaque, so iOS ships them as JPEG;
// the alpha layers stay PNG because a lossy alpha halos the fur outline.
const BACKGROUNDS = {
  plaine: 'SceneryPlaine',
  germany: 'SceneryGermany',
  finland: 'SceneryFinland',
  netherlands: 'SceneryNetherlands',
  singapore: 'ScenerySingapore',
};
const FOREGROUNDS = { terrier: 'SceneryTerrier', bula: 'SceneryBula' };
const ALL = { ...BACKGROUNDS, ...FOREGROUNDS };

const sha = (file: string) => createHash('sha256').update(readFileSync(file)).digest('hex');

// Minimal header readers, so the canvas invariant is checked without pulling an
// image decoder into the test runner.
function pngSize(buf: Buffer) {
  return { width: buf.readUInt32BE(16), height: buf.readUInt32BE(20) };
}

function jpegSize(buf: Buffer) {
  let i = 2; // skip SOI
  while (i < buf.length) {
    if (buf[i] !== 0xff) throw new Error(`not a JPEG marker at ${i}`);
    const marker = buf[i + 1];
    const length = buf.readUInt16BE(i + 2);
    // SOF0..SOF15, excluding the DHT/JPG/DAC markers that share the range.
    if (marker >= 0xc0 && marker <= 0xcf && marker !== 0xc4 && marker !== 0xc8 && marker !== 0xcc) {
      return { height: buf.readUInt16BE(i + 5), width: buf.readUInt16BE(i + 7) };
    }
    i += 2 + length;
  }
  throw new Error('no JPEG frame header');
}

function webpSize(buf: Buffer) {
  const format = buf.toString('ascii', 12, 16);
  if (format === 'VP8X') {
    return {
      width: 1 + (buf.readUIntLE(24, 3) & 0xffffff),
      height: 1 + (buf.readUIntLE(27, 3) & 0xffffff),
    };
  }
  if (format === 'VP8L') {
    const bits = buf.readUInt32LE(21);
    return { width: 1 + (bits & 0x3fff), height: 1 + ((bits >> 14) & 0x3fff) };
  }
  // Lossy VP8: 14-byte frame tag, then the 16-bit dimensions.
  return { width: buf.readUInt16LE(26) & 0x3fff, height: buf.readUInt16LE(28) & 0x3fff };
}

const iosImage = (imageset: string) => {
  const dir = path.join(IOS, `${imageset}.imageset`);
  const contents = JSON.parse(readFileSync(path.join(dir, 'Contents.json'), 'utf8'));
  const filename = contents.images[0].filename as string;
  return { dir, filename, file: path.join(dir, filename) };
};

describe('scenery assets stay registered across the three clients', () => {
  it.each(Object.keys(ALL))('%s ships on desktop, android and iOS', (slug) => {
    const desktop = path.join(DESKTOP, `${slug}.webp`);
    const android = path.join(ANDROID, `scenery_${slug}.webp`);

    // Android consumes the very same WebP the desktop does. Byte identity is
    // what catches "regenerated one platform and forgot the other".
    expect(sha(android)).toBe(sha(desktop));
    expect(webpSize(readFileSync(desktop))).toEqual(CANVAS);

    const ios = iosImage(ALL[slug as keyof typeof ALL]);
    const buf = readFileSync(ios.file);
    expect(ios.filename.startsWith(`${slug}.`)).toBe(true);
    expect(ios.filename.endsWith('.png') ? pngSize(buf) : jpegSize(buf)).toEqual(CANVAS);
  });

  it('iOS backgrounds are JPEG, so the catalog does not store them lossless', () => {
    // The masters are themselves JPEG, so a PNG imageset conserves nothing and
    // cost 10MB of Assets.car. actool stores the JPEG verbatim, no re-encode.
    for (const imageset of Object.values(BACKGROUNDS)) {
      expect(iosImage(imageset).filename).toMatch(/\.jpg$/);
    }
  });

  it('iOS alpha layers stay PNG, so the fur outline keeps its soft edge', () => {
    for (const imageset of Object.values(FOREGROUNDS)) {
      expect(iosImage(imageset).filename).toMatch(/\.png$/);
    }
  });

  it('an imageset holds exactly the one file its Contents.json names', () => {
    // A format switch that leaves the previous file behind still gets compiled
    // into the catalog, silently paying for both.
    for (const imageset of Object.values(ALL)) {
      const { dir, filename } = iosImage(imageset);
      const stray = readdirSync(dir).filter((f) => f !== 'Contents.json' && f !== filename);
      expect(stray).toEqual([]);
    }
  });
});
