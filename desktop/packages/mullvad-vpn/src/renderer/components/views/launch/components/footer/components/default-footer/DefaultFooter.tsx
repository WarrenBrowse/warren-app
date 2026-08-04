import { messages } from '../../../../../../../../shared/gettext';
import { Button } from '../../../../../../../lib/components';
import { FlexColumn } from '../../../../../../../lib/components/flex-column';
import { useBoolean } from '../../../../../../../lib/utility-hooks';
import { TroubleshootingModal } from '../../../troubleshooting-modal';
import { FooterText } from '../footer-text';
import { UnblockNetworkButton } from '../unblock-network-button';

export function DefaultLaunchFooter() {
  const [dialogOpen, showDialog, hideDialog] = useBoolean();

  return (
    <>
      <FlexColumn gap="medium">
        <FooterText>
          {
            // TRANSLATORS: Message in launch view when the mullvad service cannot be contacted.
            messages.pgettext(
              'launch-view',
              'Unable to contact the Warren system service, your connection might be unsecure. Please troubleshoot by clicking the "Learn more" button, or report the problem on our community forum.',
            )
          }
        </FooterText>
        <Button onClick={showDialog}>
          <Button.Text>{messages.gettext('Learn more')}</Button.Text>
        </Button>
        <UnblockNetworkButton />
      </FlexColumn>
      <TroubleshootingModal isOpen={dialogOpen} onClose={hideDialog} />
    </>
  );
}
