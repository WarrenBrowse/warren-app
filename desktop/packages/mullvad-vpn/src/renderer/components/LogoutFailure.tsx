import styled from 'styled-components';

import { LogoutResult } from '../../shared/daemon-rpc-types';
import { messages } from '../../shared/gettext';
import { Button, Flex } from '../lib/components';
import { colors } from '../lib/foundations';

const FailureText = styled.span({
  display: 'block',
  color: colors.red,
  fontSize: '13px',
  lineHeight: 1.4,
});

/** Why a logout left the account logged in, `undefined` when it did not. */
export function logoutFailureMessage(result: LogoutResult | undefined): string | undefined {
  switch (result) {
    case 'tunnel-still-up':
      return messages.pgettext(
        'account-view',
        'Could not log out: the VPN did not disconnect in time. You are still logged in.',
      );
    case 'failed':
      return messages.pgettext('account-view', 'Could not log out. You are still logged in.');
    default:
      return undefined;
  }
}

interface LogoutFailureProps {
  result: LogoutResult | undefined;
  onRetry: () => void;
}

/**
 * A logout the daemon refused changed nothing, so the view the user logged
 * out from stays up; this says why, next to the action, and offers the retry.
 */
export function LogoutFailure({ result, onRetry }: LogoutFailureProps) {
  const message = logoutFailureMessage(result);
  if (message === undefined) {
    return null;
  }
  return (
    <Flex flexDirection="column" gap="small" data-testid="logout-failure">
      <FailureText role="alert" aria-live="assertive">
        {message}
      </FailureText>
      <Button onClick={onRetry}>
        <Button.Text>
          {
            // TRANSLATORS: Button that runs a logout again after it failed.
            messages.pgettext('account-view', 'Try again')
          }
        </Button.Text>
      </Button>
    </Flex>
  );
}
