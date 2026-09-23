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
  const daemonAccessRefusal = useSelector((state) => state.userInterface.daemonAccessRefusal);
  if (daemonAccessRefusal !== null) return <AccessDeniedFooter refusal={daemonAccessRefusal} />;
  if (platform === 'darwin' && daemonAllowed === false) return <MacOsPermissionFooter />;
  if (platform === 'win32') return <RestartDaemonFooter />;
  return <DefaultLaunchFooter />;
};
