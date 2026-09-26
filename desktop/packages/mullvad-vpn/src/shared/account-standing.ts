import { sprintf } from 'sprintf-js';

import {
  WarrenAbuseCategory,
  WarrenAccountBan,
  WarrenAccountStanding,
  WarrenAccountStrike,
} from './daemon-rpc-types';
import { messages } from './gettext';

/**
 * How the app words the wallet's port-forward abuse standing (warren-core doc
 * 105): the warning a strike raises, the day it was recorded, and the date a
 * ban lapses. Shared by the system notification the main process raises, the
 * banner and the port-forwarding view, so the three always say the same thing.
 */

/** Where a warning is contested: the address the reports page gives. */
export const ABUSE_CONTACT = 'abuse@warrenbrowse.com';

export function abuseCategoryLabel(category: WarrenAbuseCategory): string {
  switch (category) {
    case 'copyright':
      // TRANSLATORS: Category of an abuse report: a copyright infringement notice.
      return messages.pgettext('port-forwarding-view', 'copyright');
    case 'malware-c2':
      // TRANSLATORS: Category of an abuse report: malware distribution or
      // TRANSLATORS: command and control.
      return messages.pgettext('port-forwarding-view', 'malware');
    case 'spam':
      // TRANSLATORS: Category of an abuse report.
      return messages.pgettext('port-forwarding-view', 'spam');
    case 'scanning':
      // TRANSLATORS: Category of an abuse report: scanning or intrusion attempts.
      return messages.pgettext('port-forwarding-view', 'scanning or intrusion');
    case 'phishing':
      // TRANSLATORS: Category of an abuse report.
      return messages.pgettext('port-forwarding-view', 'phishing');
    case 'csam':
      // TRANSLATORS: Category of an abuse report: child sexual abuse material.
      return messages.pgettext('port-forwarding-view', 'child sexual abuse material');
    case 'other':
    default:
      // TRANSLATORS: Category of an abuse report that fits no other category.
      return messages.pgettext('port-forwarding-view', 'other');
  }
}

/**
 * A day as the reader writes it. A strike is recorded at day precision, as
 * midnight UTC, so it is formatted in UTC: formatted in local time, a strike
 * would move to the day before for everyone west of Greenwich.
 */
export function formatStandingDay(unixSecs: number, locale: string): string {
  return new Intl.DateTimeFormat(locale, { dateStyle: 'long', timeZone: 'UTC' }).format(
    new Date(unixSecs * 1000),
  );
}

/**
 * The warning a strike raises: "Warning 1 of 3: port N was closed on DAY after
 * an abuse report (category)."
 */
export function strikeWarning(
  strike: WarrenAccountStrike,
  ordinal: number,
  threshold: number,
  locale: string,
): string {
  const values = {
    ordinal,
    threshold,
    port: strike.port,
    day: formatStandingDay(strike.dayUnixSecs, locale),
    category: abuseCategoryLabel(strike.category),
  };
  if (threshold === 0) {
    return sprintf(
      // TRANSLATORS: A warning on the account, when the number that revokes it
      // TRANSLATORS: is not known yet. Available placeholders:
      // TRANSLATORS: %(ordinal)d - the rank of this warning, from 1
      // TRANSLATORS: %(port)d - the forwarded public port that was closed
      // TRANSLATORS: %(day)s - the day it was closed, e.g. 24 September 2026
      // TRANSLATORS: %(category)s - the kind of abuse reported, e.g. copyright
      messages.pgettext(
        'port-forwarding-view',
        'Warning %(ordinal)d: port %(port)d was closed on %(day)s after an abuse report (%(category)s).',
      ),
      values,
    );
  }
  return sprintf(
    // TRANSLATORS: A warning on the account. Available placeholders:
    // TRANSLATORS: %(ordinal)d - the rank of this warning, from 1
    // TRANSLATORS: %(threshold)d - the number of warnings that revokes the account
    // TRANSLATORS: %(port)d - the forwarded public port that was closed
    // TRANSLATORS: %(day)s - the day it was closed, e.g. 24 September 2026
    // TRANSLATORS: %(category)s - the kind of abuse reported, e.g. copyright
    messages.pgettext(
      'port-forwarding-view',
      'Warning %(ordinal)d of %(threshold)d: port %(port)d was closed on %(day)s after an abuse report (%(category)s).',
    ),
    values,
  );
}

/** The reference a contest quotes. */
export function strikeCaseReference(strike: WarrenAccountStrike): string {
  return sprintf(
    // TRANSLATORS: The reference of the abuse case behind a warning.
    // TRANSLATORS: Available placeholder:
    // TRANSLATORS: %(reference)s - the case reference, e.g. PF-2026-0042
    messages.pgettext('port-forwarding-view', 'Case reference: %(reference)s'),
    { reference: strike.caseReference },
  );
}

/** How to contest a warning. */
export function strikeContestHint(): string {
  return sprintf(
    // TRANSLATORS: How to contest a warning. Available placeholder:
    // TRANSLATORS: %(email)s - the address of the abuse desk
    messages.pgettext(
      'port-forwarding-view',
      'To contest a warning, write to %(email)s quoting its case reference.',
    ),
    { email: ABUSE_CONTACT },
  );
}

/** The newest strike, with its rank among the live ones. */
export function latestStrike(
  standing: WarrenAccountStanding | null | undefined,
): { strike: WarrenAccountStrike; ordinal: number } | undefined {
  const strikes = standing?.strikes ?? [];
  if (strikes.length === 0) {
    return undefined;
  }
  return { strike: strikes[strikes.length - 1], ordinal: strikes.length };
}

/**
 * The key a dismissed warning is remembered under in the GUI settings: a
 * digest of the case reference (djb2), so the settings file names no case.
 */
export function strikeDismissalKey(strike: WarrenAccountStrike): string {
  let hash = 5381;
  for (let i = 0; i < strike.caseReference.length; i++) {
    hash = ((hash << 5) + hash + strike.caseReference.charCodeAt(i)) | 0;
  }
  return `strike:${(hash >>> 0).toString(16)}`;
}

/** The ban in force at `nowMs`, if any. */
export function banInForce(
  standing: WarrenAccountStanding | null | undefined,
  nowMs: number,
): WarrenAccountBan | undefined {
  const ban = standing?.ban ?? undefined;
  if (ban === undefined) {
    return undefined;
  }
  return ban.lapsesAtUnixSecs === null || nowMs < ban.lapsesAtUnixSecs * 1000 ? ban : undefined;
}
