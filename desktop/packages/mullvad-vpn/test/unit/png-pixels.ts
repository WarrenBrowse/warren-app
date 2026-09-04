import { inflateSync } from 'zlib';

// A minimal PNG reader, so an asset's actual colours can be asserted without
// pulling an image decoder into the test runner. The desktop suite runs on a
// Node-only CI machine with `ignore-scripts=true`, which rules out every
// decoder that ships a native build. `zlib` is part of Node.
//
// Only what ImageMagick writes into the icon trees is supported: truecolour and
// greyscale at 8 bits per sample, with or without alpha, and indexed at 1 to 8
// bits, none of it interlaced. Anything else throws rather than returning a
// wrong answer.

interface Header {
  width: number;
  height: number;
  bitDepth: number;
  colourType: number;
  interlace: number;
}

interface Image {
  header: Header;
  data: Buffer;
  plte?: Buffer;
  trns?: Buffer;
}

const SIGNATURE = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);

/** Samples per pixel, by IHDR colour type. Type 3 stores one palette index. */
const SAMPLES: Record<number, number> = { 0: 1, 2: 3, 3: 1, 4: 2, 6: 4 };

const INDEXED = 3;

function supported(header: Header): boolean {
  if (header.interlace !== 0 || !(header.colourType in SAMPLES)) {
    return false;
  }
  return header.colourType === INDEXED
    ? [1, 2, 4, 8].includes(header.bitDepth)
    : header.bitDepth === 8;
}

function readChunks(buf: Buffer): Image {
  if (!buf.subarray(0, 8).equals(SIGNATURE)) {
    throw new Error('not a PNG');
  }
  let header: Header | undefined;
  let plte: Buffer | undefined;
  let trns: Buffer | undefined;
  const idat: Buffer[] = [];
  let offset = 8;
  while (offset + 8 <= buf.length) {
    const length = buf.readUInt32BE(offset);
    const type = buf.toString('ascii', offset + 4, offset + 8);
    const body = buf.subarray(offset + 8, offset + 8 + length);
    if (type === 'IHDR') {
      header = {
        width: body.readUInt32BE(0),
        height: body.readUInt32BE(4),
        bitDepth: body[8],
        colourType: body[9],
        interlace: body[12],
      };
    } else if (type === 'PLTE') {
      plte = Buffer.from(body);
    } else if (type === 'tRNS') {
      trns = Buffer.from(body);
    } else if (type === 'IDAT') {
      idat.push(body);
    } else if (type === 'IEND') {
      break;
    }
    offset += 12 + length;
  }
  if (!header) {
    throw new Error('PNG without an IHDR');
  }
  if (!supported(header)) {
    throw new Error(
      `unsupported PNG: depth ${header.bitDepth}, type ${header.colourType}, interlace ${header.interlace}`,
    );
  }
  if (header.colourType === INDEXED && !plte) {
    throw new Error('indexed PNG without a palette');
  }
  return { header, data: inflateSync(Buffer.concat(idat)), plte, trns };
}

function paeth(a: number, b: number, c: number): number {
  const p = a + b - c;
  const pa = Math.abs(p - a);
  const pb = Math.abs(p - b);
  const pc = Math.abs(p - c);
  return pa <= pb && pa <= pc ? a : pb <= pc ? b : c;
}

/**
 * Reverses the per-scanline filters, returning the packed samples row by row.
 * Filtering works on whole bytes, so its offset is the pixel width rounded up,
 * which is 1 for every sub-byte indexed image.
 */
function unfilter(data: Buffer, header: Header, stride: number, offset: number): Buffer {
  const out = Buffer.alloc(stride * header.height);
  for (let y = 0; y < header.height; y++) {
    const filter = data[y * (stride + 1)];
    const line = data.subarray(y * (stride + 1) + 1, y * (stride + 1) + 1 + stride);
    for (let x = 0; x < stride; x++) {
      const left = x >= offset ? out[y * stride + x - offset] : 0;
      const up = y > 0 ? out[(y - 1) * stride + x] : 0;
      const upLeft = y > 0 && x >= offset ? out[(y - 1) * stride + x - offset] : 0;
      let value = line[x];
      switch (filter) {
        case 0:
          break;
        case 1:
          value += left;
          break;
        case 2:
          value += up;
          break;
        case 3:
          value += (left + up) >> 1;
          break;
        case 4:
          value += paeth(left, up, upLeft);
          break;
        default:
          throw new Error(`unknown PNG filter ${filter}`);
      }
      out[y * stride + x] = value & 0xff;
    }
  }
  return out;
}

const hex = (r: number, g: number, b: number) =>
  '#' + [r, g, b].map((c) => c.toString(16).padStart(2, '0').toUpperCase()).join('');

/**
 * The colours a PNG paints solidly, as uppercase `#RRGGBB`.
 *
 * A pixel counts only when it is fully opaque and its four neighbours carry the
 * same colour. Antialiasing, and the seam where two opaque layers meet, produce
 * a fringe of blends as opaque as the fill and, on a 22px icon, nearly as
 * numerous: the dot of the connecting frame rings itself in a colour that is in
 * neither layer. Eroding that fringe away leaves the colours the icon is drawn
 * in. `minShare` then drops a stray speck that survived the erosion.
 */
export function dominantColours(buf: Buffer, minShare = 0.005): Set<string> {
  const { header, data, plte, trns } = readChunks(buf);
  const { width, height, bitDepth, colourType } = header;
  const bitsPerPixel = SAMPLES[colourType] * bitDepth;
  const stride = Math.ceil((width * bitsPerPixel) / 8);
  const samples = unfilter(data, header, stride, Math.ceil(bitsPerPixel / 8));
  const hasAlpha = colourType === 4 || colourType === 6;
  const greyscale = colourType === 0 || colourType === 4;
  const perByte = 8 / bitDepth;
  const mask = (1 << bitDepth) - 1;

  const at = (x: number, y: number): string | undefined => {
    if (x < 0 || y < 0 || x >= width || y >= height) {
      return undefined;
    }
    if (colourType === INDEXED) {
      const byte = samples[y * stride + Math.floor(x / perByte)];
      const shift = bitDepth * (perByte - 1 - (x % perByte));
      const entry = (byte >> shift) & mask;
      // tRNS gives an alpha to the leading entries only; the rest are opaque.
      if (trns && entry < trns.length && trns[entry] !== 0xff) {
        return undefined;
      }
      return hex(plte![entry * 3], plte![entry * 3 + 1], plte![entry * 3 + 2]);
    }
    const index = y * stride + x * SAMPLES[colourType];
    if (hasAlpha && samples[index + SAMPLES[colourType] - 1] !== 0xff) {
      return undefined;
    }
    return greyscale
      ? hex(samples[index], samples[index], samples[index])
      : hex(samples[index], samples[index + 1], samples[index + 2]);
  };

  const counts = new Map<string, number>();
  let solid = 0;
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      const colour = at(x, y);
      if (colour === undefined) {
        continue;
      }
      const surrounded = [at(x - 1, y), at(x + 1, y), at(x, y - 1), at(x, y + 1)].every(
        (neighbour) => neighbour === colour,
      );
      if (!surrounded) {
        continue;
      }
      solid += 1;
      counts.set(colour, (counts.get(colour) ?? 0) + 1);
    }
  }

  const dominant = new Set<string>();
  for (const [colour, count] of counts) {
    if (count >= solid * minShare) {
      dominant.add(colour);
    }
  }
  return dominant;
}
