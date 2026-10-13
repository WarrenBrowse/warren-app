import fs from 'fs';
import os from 'os';
import path from 'path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';

import { RenewalNotice } from '../../src/main/renewal-flow';
import {
  RenewalNoticeArchive,
  renewalNoticeDocument,
  renewalNoticeHtml,
} from '../../src/main/renewal-notice';

const DAY_MS = 86_400_000;
const RENEWS_AT = Date.UTC(2026, 11, 1);
const SHOWN_AT = RENEWS_AT - 35 * DAY_MS;

function notice(overrides: Partial<RenewalNotice> = {}): RenewalNotice {
  return {
    renewsAtMs: RENEWS_AT,
    shownAtMs: SHOWN_AT,
    currency: 'EUR',
    monthlyCents: 700,
    bestMonthlyCents: 650,
    ...overrides,
  };
}

// The notice is the durable record of the end of a commitment (GEO 111/2011
// art. 50, Lexter memo para 36(e)): the date, the amount, the best tariff and
// the way out must all be on it.
describe('renewalNoticeDocument', () => {
  it('names the renewal date, the monthly price, the best tariff and how to cancel', () => {
    const doc = renewalNoticeDocument(notice(), 'en');
    const text = doc.lines.join('\n');

    expect(doc.title).toBe('Automatic renewal notice');
    expect(text).toContain(new Date(SHOWN_AT).toLocaleDateString('en'));
    expect(text).toContain(new Date(RENEWS_AT).toLocaleDateString('en'));
    expect(text).toContain('€7.00');
    expect(text).toContain('€6.50');
    expect(text).toContain('https://checkout.warrenbrowse.com/resiliation');
  });

  it('names the previous and the new price when the notice announces a raise', () => {
    const text = renewalNoticeDocument(
      notice({ monthlyCents: 800, previousMonthlyCents: 700 }),
      'en',
    ).lines.join('\n');

    expect(text).toContain('€7.00');
    expect(text).toContain('€8.00');
  });

  it('escapes what it puts in the HTML it prints', () => {
    const html = renewalNoticeHtml({ title: 'a<b', lines: ['x & y'] });

    expect(html).toContain('a&lt;b');
    expect(html).toContain('x &amp; y');
    expect(html).not.toContain('a<b');
  });
});

describe('RenewalNoticeArchive', () => {
  let dir: string;

  beforeEach(() => {
    dir = fs.mkdtempSync(path.join(os.tmpdir(), 'renewal-notices-'));
  });

  afterEach(() => {
    fs.rmSync(dir, { recursive: true, force: true });
  });

  it('keeps every notice as a dated PDF and lists them newest first', async () => {
    const archive = new RenewalNoticeArchive(dir);
    await archive.save(notice({ shownAtMs: SHOWN_AT }), Buffer.from('%PDF-1'));
    await archive.save(notice({ shownAtMs: SHOWN_AT + DAY_MS }), Buffer.from('%PDF-2'));

    const entries = archive.list();

    expect(entries.map((e) => e.shownAtMs)).toEqual([SHOWN_AT + DAY_MS, SHOWN_AT]);
    expect(entries[0].renewsAtMs).toBe(RENEWS_AT);
    expect(fs.readFileSync(archive.pathOf(entries[1].id)!).toString()).toBe('%PDF-1');
  });

  it('resolves only the files it wrote, never a path a caller makes up', () => {
    const archive = new RenewalNoticeArchive(dir);

    expect(archive.pathOf('../../etc/passwd')).toBeUndefined();
    expect(archive.pathOf('renewal-notice-1-2.pdf')).toBeUndefined();
  });
});
