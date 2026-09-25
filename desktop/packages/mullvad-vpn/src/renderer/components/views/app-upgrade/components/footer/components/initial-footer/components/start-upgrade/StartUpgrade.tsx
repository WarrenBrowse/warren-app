import { messages } from '../../../../../../../../../../shared/gettext';
import { Button, Flex } from '../../../../../../../../../lib/components';
import { useVersionSuggestedUpgrade } from '../../../../../../../../../redux/hooks';
import { useOpenDownloadUrl } from '../../../../../../../app-info/components/update-available-list-item/hooks/useOpenDownloadUrl';
import { UpgradeButton } from '../../../../../upgrade-button';
import { useDisabled } from './hooks';

export function StartUpgrade() {
  const disabled = useDisabled();
  const { suggestedUpgrade } = useVersionSuggestedUpgrade();
  const openDownloadUrl = useOpenDownloadUrl();

  const label =
    // TRANSLATORS: Button text to download and install an update
    messages.pgettext('app-upgrade-view', 'Download & install');

  return (
    <Flex padding="large" flexDirection="column">
      <Flex flexDirection="column">
        {suggestedUpgrade?.manualInstallOnly ? (
          // No published installer fits this install (a Linux system no
          // supported package manager owns): the website is the only way.
          <Button onClick={openDownloadUrl}>
            <Button.Text>{label}</Button.Text>
          </Button>
        ) : (
          <UpgradeButton disabled={disabled}>{label}</UpgradeButton>
        )}
      </Flex>
    </Flex>
  );
}
