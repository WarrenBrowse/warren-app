import { useCallback } from 'react';

import { messages } from '../../../../shared/gettext';
import { RoutePath } from '../../../../shared/routes';
import { IconButton, IconButtonProps, MainHeader } from '../../../lib/components';
import { TransitionType, useHistory } from '../../../lib/history';
import { useSelector } from '../../../redux/store';

export type MainHeaderBarAccountButtonProps = Omit<IconButtonProps, 'icon'>;

export const AppMainHeaderBarAccountButton = (props: MainHeaderBarAccountButtonProps) => {
  const history = useHistory();
  const openAccount = useCallback(
    () => history.push(RoutePath.account, { transition: TransitionType.show }),
    [history],
  );

  const loggedIn = useSelector((state) => state.account.status.type === 'ok');
  if (!loggedIn) {
    return null;
  }

  return (
    <MainHeader.IconButton
      onClick={openAccount}
      data-testid="account-button"
      aria-label={messages.gettext('Account settings')}
      {...props}>
      {/* Outline rather than filled: the header floats over the artwork next to
          the thin-stroke wordmark, and solid glyphs read heavier than the logo.
          An open bust (no enclosing circle) so the initial keyboard-focus ring
          does not read as a second circle around the glyph. */}
      <IconButton.Icon icon="account-outline" />
    </MainHeader.IconButton>
  );
};
