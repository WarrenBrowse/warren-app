import fs from 'fs';
import path from 'path';
import { sprintf } from 'sprintf-js';

import { urls } from '../shared/constants/urls';
import { messages } from '../shared/gettext';
import { formatMinorUnits, RenewalNotice } from '../shared/renewal';

// The pre-renewal notice on a durable medium (GEO 111/2011 art. 50, Lexter
// memo para 36(e)): every notice the app shows is also written as a dated PDF
// the user can open, keep and print, and the app lists them in Account
// settings. The PDF is built from plain text so the same document is readable
// on every platform and in every locale.

export interface RenewalNoticeDocument {
  title: string;
  lines: string[];
}

export function renewalNoticeDocument(
  notice: RenewalNotice,
  locale: string,
): RenewalNoticeDocument {
  const date = (ms: number) => new Date(ms).toLocaleDateString(locale);
  const price = (cents: number | undefined) =>
    cents === undefined ? undefined : formatMinorUnits(cents, notice.currency, locale);

  const lines: string[] = [
    sprintf(
      // TRANSLATORS: First line of the renewal notice document.
      // TRANSLATORS: Available placeholder:
      // TRANSLATORS: %(date)s - the date the notice was issued
      messages.pgettext('notifications', 'Issued on %(date)s'),
      { date: date(notice.shownAtMs) },
    ),
    sprintf(
      // TRANSLATORS: Renewal notice document: the next automatic renewal date.
      // TRANSLATORS: Available placeholder:
      // TRANSLATORS: %(date)s - the renewal date
      messages.pgettext(
        'notifications',
        'Your Warren time renews automatically on or around %(date)s, for one month.',
      ),
      { date: date(notice.renewsAtMs) },
    ),
  ];
  const monthly = price(notice.monthlyCents);
  const previous = price(notice.previousMonthlyCents);
  if (monthly !== undefined && previous !== undefined) {
    lines.push(
      sprintf(
        // TRANSLATORS: Renewal notice document announcing a price increase.
        // TRANSLATORS: Available placeholders:
        // TRANSLATORS: %(previous)s - the former monthly price
        // TRANSLATORS: %(price)s - the new monthly price
        messages.pgettext(
          'notifications',
          'The monthly price changes from %(previous)s to %(price)s, VAT included, from this renewal.',
        ),
        { previous, price: monthly },
      ),
    );
  } else if (monthly !== undefined) {
    lines.push(
      sprintf(
        // TRANSLATORS: Renewal notice document: the amount that will be charged.
        // TRANSLATORS: Available placeholder:
        // TRANSLATORS: %(price)s - the monthly price
        messages.pgettext('notifications', 'Amount charged: %(price)s, VAT included.'),
        { price: monthly },
      ),
    );
  }
  const best = price(notice.bestMonthlyCents);
  if (best !== undefined) {
    lines.push(
      sprintf(
        // TRANSLATORS: Renewal notice document: the lowest monthly price any plan offers.
        // TRANSLATORS: Available placeholder:
        // TRANSLATORS: %(price)s - the best monthly price
        messages.pgettext('notifications', 'Best monthly rate Warren offers today: %(price)s.'),
        { price: best },
      ),
    );
  }
  lines.push(
    sprintf(
      // TRANSLATORS: Renewal notice document: how to cancel, free of charge.
      // TRANSLATORS: Available placeholder:
      // TRANSLATORS: %(url)s - the page to cancel the contract
      messages.pgettext(
        'notifications',
        'You can turn automatic renewal off at any time, at no cost, in Account settings of the Warren app or at %(url)s. Turning it off takes effect immediately.',
      ),
      { url: urls.cancelContract },
    ),
  );
  return {
    // TRANSLATORS: Title of the renewal notice document.
    title: messages.pgettext('notifications', 'Automatic renewal notice'),
    lines,
  };
}

function escapeHtml(text: string): string {
  return text
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

export function renewalNoticeHtml(doc: RenewalNoticeDocument): string {
  const body = doc.lines.map((line) => `<p>${escapeHtml(line)}</p>`).join('\n');
  return [
    '<!doctype html>',
    '<html><head><meta charset="utf-8">',
    `<title>${escapeHtml(doc.title)}</title>`,
    '<style>body{font-family:sans-serif;margin:48px;color:#111;line-height:1.5}h1{font-size:20px}</style>',
    '</head><body>',
    `<h1>${escapeHtml(doc.title)}</h1>`,
    body,
    '<p>Warren VPN</p>',
    '</body></html>',
  ].join('\n');
}

export interface RenewalNoticeEntry {
  id: string;
  shownAtMs: number;
  renewsAtMs: number;
}

const FILE_PATTERN = /^renewal-notice-(\d{1,15})-(\d{1,15})\.pdf$/;

export class RenewalNoticeArchive {
  public constructor(private dir: string) {}

  public async save(notice: RenewalNotice, pdf: Buffer): Promise<RenewalNoticeEntry> {
    await fs.promises.mkdir(this.dir, { recursive: true });
    const id = `renewal-notice-${notice.shownAtMs}-${notice.renewsAtMs}.pdf`;
    await fs.promises.writeFile(path.join(this.dir, id), pdf);
    return { id, shownAtMs: notice.shownAtMs, renewsAtMs: notice.renewsAtMs };
  }

  public list(): RenewalNoticeEntry[] {
    let names: string[];
    try {
      names = fs.readdirSync(this.dir);
    } catch {
      return [];
    }
    return names
      .map((id) => {
        const match = FILE_PATTERN.exec(id);
        return match
          ? { id, shownAtMs: Number(match[1]), renewsAtMs: Number(match[2]) }
          : undefined;
      })
      .filter((entry): entry is RenewalNoticeEntry => entry !== undefined)
      .sort((a, b) => b.shownAtMs - a.shownAtMs);
  }

  // The path of a notice this archive holds; undefined for anything else, so
  // an id coming from the renderer can never name another file.
  public pathOf(id: string): string | undefined {
    if (!FILE_PATTERN.test(id)) {
      return undefined;
    }
    const file = path.join(this.dir, id);
    return fs.existsSync(file) ? file : undefined;
  }
}
