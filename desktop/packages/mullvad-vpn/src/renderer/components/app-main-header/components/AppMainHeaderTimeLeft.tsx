import { useEffect, useState } from 'react';
import { sprintf } from 'sprintf-js';
import styled from 'styled-components';

import { closeToExpiry, formatRemainingTime, hasExpired } from '../../../../shared/account-expiry';
import { messages } from '../../../../shared/gettext';
import { LabelTinySemiBold, useMainHeaderTone } from '../../../lib/components';
import { useInterval } from '../../../lib/hooks';
import { useSelector } from '../../../redux/store';

const StyledTimeLeftLabel = styled(LabelTinySemiBold)({
  // Hug the right edge of the header row even when the left-hand pubkey slot
  // is empty (single child under justify-content: space-between).
  marginLeft: 'auto',
  whiteSpace: 'nowrap',
});

export const AppMainHeaderTimeLeft = () => {
  const accountExpiry = useSelector((state) => state.account.expiry);
  const isOutOfTime = accountExpiry ? hasExpired(accountExpiry) : false;
  const dark = useMainHeaderTone() === 'dark';

  const [timeLeft, setTimeLeft] = useState(formatTimeLeft(accountExpiry));

  useInterval(() => setTimeLeft(formatTimeLeft(accountExpiry)), 60 * 60 * 1_000);

  useEffect(() => {
    setTimeLeft(formatTimeLeft(accountExpiry));
  }, [accountExpiry]);

  if (!accountExpiry || closeToExpiry(accountExpiry) || isOutOfTime) {
    return null;
  }

  return (
    <StyledTimeLeftLabel color={dark ? 'black' : 'white'}>
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
