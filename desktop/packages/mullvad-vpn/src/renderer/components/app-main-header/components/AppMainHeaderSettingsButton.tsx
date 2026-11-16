import { useCallback } from 'react';

import { messages } from '../../../../shared/gettext';
import { RoutePath } from '../../../../shared/routes';
import { IconButton, IconButtonProps, MainHeader } from '../../../lib/components';
import { TransitionType, useHistory } from '../../../lib/history';
import { useSelector } from '../../../redux/store';
import { ToolbarBadge, ToolbarGlyph } from './ToolbarBadge';

export type MainHeaderSettingsButtonProps = Omit<IconButtonProps, 'icon'>;

export function AppMainHeaderSettingsButton(props: MainHeaderSettingsButtonProps) {
  const history = useHistory();
  const suggestedUpgrade = useSelector((state) => state.version.suggestedUpgrade);

  const openSettings = useCallback(() => {
    if (!props.disabled) {
      history.push(RoutePath.settings, { transition: TransitionType.show });
    }
  }, [history, props.disabled]);

  return (
    <MainHeader.IconButton onClick={openSettings} aria-label={messages.gettext('Settings')}>
      {/* Outline rather than filled, matching the wordmark's thin stroke. The
          outline gear is hollow, so the upgrade dot sits on it directly and the
          notched "partial" variant is unnecessary. */}
      {suggestedUpgrade ? (
        <ToolbarGlyph>
          <IconButton.Icon icon="settings-outline" />
          <ToolbarBadge />
        </ToolbarGlyph>
      ) : (
        <IconButton.Icon icon="settings-outline" />
      )}
    </MainHeader.IconButton>
  );
}
