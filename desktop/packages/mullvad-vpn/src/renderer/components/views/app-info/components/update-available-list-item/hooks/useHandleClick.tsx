import { usePushAppUpgrade } from '../../../../../../history/hooks';
import { useVersionSuggestedUpgrade } from '../../../../../../redux/hooks';
import { useOpenDownloadUrl } from './useOpenDownloadUrl';

export const useHandleClick = () => {
  const openDownloadUrl = useOpenDownloadUrl();
  const pushAppUpgrade = usePushAppUpgrade();
  const { suggestedUpgrade } = useVersionSuggestedUpgrade();

  return suggestedUpgrade?.manualInstallOnly ? openDownloadUrl : pushAppUpgrade;
};
