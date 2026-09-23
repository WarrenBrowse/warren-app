import { useSelector } from '../../../../../../../redux/store';
import {
  AccessDeniedFooter,
  DefaultLaunchFooter,
  MacOsPermissionFooter,
  RestartDaemonFooter,
} from '../../..';

export const useContent = () => {
  const platform = window.env.platform;
  const daemonAllowed = useSelector((state) => state.userInterface.daemonAllowed);
  const daemonAccessDenied = useSelector((state) => state.userInterface.daemonAccessDenied);
  if (daemonAccessDenied) return <AccessDeniedFooter />;
  if (platform === 'darwin' && daemonAllowed === false) return <MacOsPermissionFooter />;
  if (platform === 'win32') return <RestartDaemonFooter />;
  return <DefaultLaunchFooter />;
};
