import { messages } from '../../../../../../../shared/gettext';
import { BodySmall, Flex, Spinner } from '../../../../../../lib/components';
import { FlexColumn } from '../../../../../../lib/components/flex-column';
import { useUserInterfaceDaemonStatus } from '../../../../../../redux/hooks';
import { useSelector } from '../../../../../../redux/store';

export const useStatusText = () => {
  const { daemonStatus } = useUserInterfaceDaemonStatus();
  const daemonAccessRefusal = useSelector((state) => state.userInterface.daemonAccessRefusal);

  if (daemonAccessRefusal !== null) {
    return (
      <Flex justifyContent="center">
        <BodySmall color="whiteAlpha40" textAlign="center" role="alert">
          {daemonAccessRefusal === 'claimNeedsConsoleUser'
            ? // TRANSLATORS: Status text when Warren was set up on this computer before it recorded an owner, and this account may not take it over.
              messages.pgettext('launch-view', 'Warren on this computer has no owner yet')
            : // TRANSLATORS: Status text when another account on this computer set up Warren, so the daemon refuses this one.
              messages.pgettext(
                'launch-view',
                'Warren is set up by another account on this computer',
              )}
        </BodySmall>
      </Flex>
    );
  }

  let statusMessage = (
    <BodySmall color="whiteAlpha40" textAlign="center" role="alert">
      {
        // TRANSLATORS: Status text app is trying to connect to the system service.
        messages.pgettext('launch-view', 'Connecting to Warren system service...')
      }
    </BodySmall>
  );
  if (window.env.platform === 'win32') {
    if (daemonStatus === 'start-requested') {
      statusMessage = (
        <FlexColumn alignItems="center" gap="big">
          <BodySmall color="whiteAlpha40" textAlign="center" role="alert">
            {
              // TRANSLATORS: Status text shown when app is starting.
              messages.pgettext('launch-view', 'Starting up....')
            }
          </BodySmall>
          <Spinner size="medium" />
        </FlexColumn>
      );
    } else {
      statusMessage = (
        <BodySmall color="whiteAlpha40" textAlign="center" role="alert">
          {
            // TRANSLATORS: Status text shown when app fails to start.
            messages.pgettext(
              'launch-view',
              'Failed to start the app, please try again or click “Details” for more info',
            )
          }
        </BodySmall>
      );
    }
  }

  return <Flex justifyContent="center">{statusMessage}</Flex>;
};
