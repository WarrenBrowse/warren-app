import { sprintf } from 'sprintf-js';

import { strings } from '../../../../../../../../shared/constants';
import { messages } from '../../../../../../../../shared/gettext';
import { Icon } from '../../../../../../../lib/components';
import { FlexColumn } from '../../../../../../../lib/components/flex-column';
import { FlexRow } from '../../../../../../../lib/components/flex-row';
import { Link } from '../../../../../../../lib/components/link';
import { useLinuxSettingsContext } from '../../LinuxSettingsContext';
import { useShowUnsupportedDialog } from './hooks';

export function HeaderDescription() {
  const { launchMode, splitTunnelingSupported } = useLinuxSettingsContext();
  const message = sprintf(
    // TRANSLATORS: Information about split tunneling not being supported on the system.
    // TRANSLATORS: Available placeholders:
    // TRANSLATORS: %(splitTunneling)s - will be replaced with Split tunneling
    messages.pgettext(
      'split-tunneling-view',
      '%(splitTunneling)s is not supported by your system.',
    ),
    {
      splitTunneling: strings.splitTunneling,
    },
  );
  const showUnsupportedDialog = useShowUnsupportedDialog();

  if (splitTunnelingSupported === false) {
    return (
      <FlexRow>
        <FlexColumn justifyContent="center" margin={{ right: 'small' }}>
          <Icon size="small" color="whiteAlpha60" icon="info-circle" />
        </FlexColumn>
        <FlexColumn>
          <span>
            {message}
            &nbsp;
            <Link
              aria-description={message}
              as="button"
              onClick={showUnsupportedDialog}
              variant="labelTinySemiBold">
              <Link.Text>
                {
                  // TRANSLATORS: Link for learning more
                  messages.pgettext('split-tunneling-view', 'Click here to learn more')
                }
              </Link.Text>
            </Link>
          </span>
        </FlexColumn>
      </FlexRow>
    );
  }

  if (launchMode === 'include') {
    return messages.pgettext(
      'split-tunneling-view',
      'Click on an app to launch it. It uses the VPN until you close it, and nothing else does.',
    );
  }

  return messages.pgettext(
    'split-tunneling-view',
    'Click on an app to launch it. Its traffic will bypass the VPN tunnel until you close it.',
  );
}
