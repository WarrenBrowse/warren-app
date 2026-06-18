import { useEffect, useState } from 'react';
import { sprintf } from 'sprintf-js';
import styled from 'styled-components';

import { closeToExpiry, formatRemainingTime, hasExpired } from '../../../../shared/account-expiry';
import { messages } from '../../../../shared/gettext';
import { FootnoteMini } from '../../../lib/components';
import { useInterval } from '../../../lib/hooks';
import { useSelector } from '../../../redux/store';

const StyledTimeLeftLabel = styled(FootnoteMini)({
  // Hug the right edge of the header row even when the left-hand pubkey slot
  // is empty (single child under justify-content: space-between).
  marginLeft: 'auto',
  whiteSpace: 'nowrap',
});

// Warren has no per-device concept (removed in the Option A3 refactor), so
// this restores ONLY the "Time left" subscription readout that lived in the
// upstream Mullvad header's device-info row. Near/past expiry the dedicated
// notification banner surfaces the remaining time, so the header label hides
// then (same behaviour as upstream).
export const AppMainHeaderTimeLeft = () => {
  const accountExpiry = useSelector((state) => state.account.expiry);
  const isOutOfTime = accountExpiry ? hasExpired(accountExpiry) : false;

  const [timeLeft, setTimeLeft] = useState(formatTimeLeft(accountExpiry));

  // The time-left value must be recomputed recurringly since it changes as
  // time passes.
  useInterval(() => setTimeLeft(formatTimeLeft(accountExpiry)), 60 * 60 * 1_000);

  // ...and whenever the account expiry itself changes (e.g. after redeeming
  // a voucher).
  useEffect(() => {
    setTimeLeft(formatTimeLeft(accountExpiry));
  }, [accountExpiry]);

  if (!accountExpiry || closeToExpiry(accountExpiry) || isOutOfTime) {
    return null;
  }

  return (
    <StyledTimeLeftLabel color="whiteAlpha80">
      {sprintf(
        // TRANSLATORS: Label in the main header showing the remaining
        // TRANSLATORS: subscription time.
        // TRANSLATORS: Available placeholders:
        // TRANSLATORS: %(timeLeft)s - the remaining time, e.g. "29 days"
        messages.pgettext('device-management', 'Time left: %(timeLeft)s'),
        { timeLeft },
      )}
    </StyledTimeLeftLabel>
  );
};

function formatTimeLeft(accountExpiry?: string): string {
  return accountExpiry ? formatRemainingTime(accountExpiry) : '';
}
