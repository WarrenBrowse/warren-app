import { useCallback } from 'react';
import styled from 'styled-components';

import { urls } from '../../../../../../shared/constants';
import { messages } from '../../../../../../shared/gettext';
import { useAppContext } from '../../../../../context';
import { Button } from '../../../../../lib/components';
import { ModalAlert, ModalAlertType, ModalMessage, ModalMessageList } from '../../../../Modal';
import { useTroubleshootingSteps } from './hooks';

export type TroubleshootingModalProps = {
  isOpen: boolean;
  onClose: () => void;
};

const StyledModalMessage = styled(ModalMessage)`
  margin-top: 0;
`;

export function TroubleshootingModal({ isOpen, onClose }: TroubleshootingModalProps) {
  const { openUrl } = useAppContext();
  const openForum = useCallback(() => openUrl(urls.forum), [openUrl]);

  const steps = useTroubleshootingSteps();

  return (
    <ModalAlert
      isOpen={isOpen}
      type={ModalAlertType.info}
      close={onClose}
      buttons={[
        <Button variant="success" key="forum" onClick={openForum}>
          <Button.Text>
            {
              // TRANSLATORS: Button label opening the community forum to report a problem.
              messages.pgettext('launch-view', 'Report on the forum')
            }
          </Button.Text>
        </Button>,
        <Button key="back" onClick={onClose}>
          <Button.Text>{messages.gettext('Back')}</Button.Text>
        </Button>,
      ]}>
      <ModalMessage>
        {
          // TRANSLATORS: Message in troubleshooting modal when the background process failed to start.
          messages.pgettext(
            'launch-view',
            'The Warren background process failed to start. The background process is responsible for the security, kill switch, and the VPN tunnel. Please try:',
          )
        }
      </ModalMessage>
      <StyledModalMessage>
        <ModalMessageList>{steps}</ModalMessageList>
      </StyledModalMessage>
      <ModalMessage>
        {
          // TRANSLATORS: Message in troubleshooting modal pointing the user to the community forum if the steps do not work.
          messages.pgettext(
            'launch-view',
            'If these steps do not work, please report the problem on our community forum.',
          )
        }
      </ModalMessage>
    </ModalAlert>
  );
}
