import { useCallback } from 'react';

import { messages } from '../../../../shared/gettext';
import { RoutePath } from '../../../../shared/routes';
import { IconButton, IconButtonProps, MainHeader } from '../../../lib/components';
import { TransitionType, useHistory } from '../../../lib/history';

export type MainHeaderSupportButtonProps = Omit<IconButtonProps, 'icon'>;

/**
 * The lifebuoy: one click from the home screen to the bug report form, which
 * files the forum topic itself (account included) and opens it signed in. It
 * stays whatever the forum notifications setting says: asking for help is
 * not forum activity, and the form is where a reporter finds their reports.
 */
export function AppMainHeaderSupportButton(props: MainHeaderSupportButtonProps) {
  const history = useHistory();
  const openReport = useCallback(
    () => history.push(RoutePath.reportProblem, { transition: TransitionType.show }),
    [history],
  );
  return (
    <MainHeader.IconButton
      onClick={openReport}
      data-testid="report-problem-button"
      aria-label={
        // TRANSLATORS: Accessible name of the header lifebuoy that opens the bug report form.
        messages.pgettext('report-problem', 'Report a problem')
      }
      {...props}>
      {/* Same outline weight as the bell and the gear beside it. */}
      <IconButton.Icon icon="lifebuoy-outline" />
    </MainHeader.IconButton>
  );
}
