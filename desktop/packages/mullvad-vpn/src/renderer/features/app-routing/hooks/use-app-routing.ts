import React from 'react';

import { ISplitTunnelingApplication } from '../../../../shared/application-types';
import { AppSplitMode, ExitChoice } from '../../../../shared/daemon-rpc-types';
import log from '../../../../shared/logging';
import { useAppContext } from '../../../context';
import { useSelector } from '../../../redux/store';

// Failures are logged without their message: one raised while resolving a
// picked program can name its path, and here a path is tied to an exit.
export function useAppRouting() {
  const routing = useSelector((state) => state.settings.appRouting);
  const statuses = useSelector((state) => state.settings.appRouteStatus);
  const applications = useSelector((state) => state.settings.appRoutingApplications);
  const app = useAppContext();

  const setSplitMode = React.useCallback(
    async (mode: AppSplitMode) => {
      try {
        await app.setAppSplitMode(mode);
      } catch {
        log.error('Could not set the split mode');
      }
    },
    [app],
  );

  const setAppExitsEnabled = React.useCallback(
    async (enabled: boolean) => {
      try {
        await app.setAppExitsEnabled(enabled);
      } catch {
        log.error('Could not switch the per-app countries');
      }
    },
    [app],
  );

  // Answers whether the choice was applied; a failure is logged.
  const setAppExit = React.useCallback(
    async (application: ISplitTunnelingApplication | string, exit: ExitChoice) => {
      try {
        await app.setAppExit(application, exit);
        return true;
      } catch {
        log.error('Could not set the country of an app');
        return false;
      }
    },
    [app],
  );

  const clearAppExit = React.useCallback(
    async (application: string) => {
      try {
        await app.clearAppExit(application);
      } catch {
        log.error('Could not clear the country of an app');
      }
    },
    [app],
  );

  const addIncludedApp = React.useCallback(
    async (application: ISplitTunnelingApplication | string) => {
      try {
        await app.addIncludedApp(application);
      } catch {
        log.error('Could not add an app to VPN only for');
      }
    },
    [app],
  );

  const removeIncludedApp = React.useCallback(
    async (application: string) => {
      try {
        await app.removeIncludedApp(application);
      } catch {
        log.error('Could not remove an app from VPN only for');
      }
    },
    [app],
  );

  return {
    routing,
    statuses,
    applications,
    platform: window.env.platform,
    setSplitMode,
    setAppExitsEnabled,
    setAppExit,
    clearAppExit,
    addIncludedApp,
    removeIncludedApp,
  };
}
