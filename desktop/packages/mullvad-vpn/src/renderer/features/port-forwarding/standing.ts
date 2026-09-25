import { sprintf } from 'sprintf-js';

import {
  banInForce,
  formatStandingDay,
  strikeCaseReference,
  strikeWarning,
} from '../../../shared/account-standing';
import { WarrenAccountBan, WarrenAccountStanding } from '../../../shared/daemon-rpc-types';
import { messages } from '../../../shared/gettext';

/** What the port-forwarding view shows of the wallet's standing. */
export interface StandingSection {
  ban?: string;
  warnings: Array<{ text: string; reference: string }>;
}

/**
 * The standing section of the port-forwarding view, `undefined` when there is
 * nothing to say (good standing, or nothing known yet). Pure, so every case is
 * pinned without rendering a component.
 */
export function standingSection(
  standing: WarrenAccountStanding | null,
  locale: string,
  nowMs: number,
): StandingSection | undefined {
  if (standing === null) {
    return undefined;
  }
  const ban = banInForce(standing, nowMs);
  if (ban === undefined && standing.strikes.length === 0) {
    return undefined;
  }
  return {
    ban: ban === undefined ? undefined : banLine(ban, locale),
    warnings: standing.strikes.map((strike, index) => ({
      text: strikeWarning(strike, index + 1, standing.threshold, locale),
      reference: strikeCaseReference(strike),
    })),
  };
}

function banLine(ban: WarrenAccountBan, locale: string): string {
  const until =
    ban.lapsesAtUnixSecs === null ? undefined : formatStandingDay(ban.lapsesAtUnixSecs, locale);
  if (ban.reason === 'port-forwarding-abuse') {
    return until === undefined
      ? messages.pgettext('port-forwarding-view', 'Access suspended for port-forwarding abuse.')
      : sprintf(
          // TRANSLATORS: Available placeholder:
          // TRANSLATORS: %(date)s - the day the suspension ends, e.g. 24 September 2027
          messages.pgettext(
            'port-forwarding-view',
            'Access suspended for port-forwarding abuse until %(date)s.',
          ),
          { date: until },
        );
  }
  return until === undefined
    ? messages.pgettext('port-forwarding-view', 'Access suspended.')
    : sprintf(
        // TRANSLATORS: Available placeholder:
        // TRANSLATORS: %(date)s - the day the suspension ends, e.g. 24 September 2027
        messages.pgettext('port-forwarding-view', 'Access suspended until %(date)s.'),
        { date: until },
      );
}
