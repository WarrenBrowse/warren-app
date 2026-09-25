import React from 'react';

import { ISplitTunnelingApplication } from '../../../../shared/application-types';
import { AppSplitMode, ExitChoice, SetAppExitOutcome } from '../../../../shared/daemon-rpc-types';
import log from '../../../../shared/logging';
import { useAppContext } from '../../../context';
import { useSelector } from '../../../redux/store';

export function useAppRouting() {
  const routing = useSelector((state) => state.settings.appRouting);
  const statuses = useSelector((state) => state.settings.appRouteStatus);
  const applications = useSelector((state) => state.settings.appRoutingApplications);
  const app = useAppContext();

  const setSplitMode = React.useCallback(
    async (mode: AppSplitMode) => {
      try {
        await app.setAppSplitMode(mode);
      } catch (e) {
        log.error('Could not set the split mode', (e as Error).message);
      }
    },
    [app],
  );

  const setAppExitsEnabled = React.useCallback(
    async (enabled: boolean) => {
      try {
        await app.setAppExitsEnabled(enabled);
      } catch (e) {
        log.error('Could not switch the per-app countries', (e as Error).message);
      }
    },
    [app],
  );

  // Answers the daemon's verdict so the picker can explain a refusal; any
  // other failure is logged and reported as not applied.
  const setAppExit = React.useCallback(
    async (
      application: ISplitTunnelingApplication | string,
      exit: ExitChoice,
    ): Promise<SetAppExitOutcome | undefined> => {
      try {
        return await app.setAppExit(application, exit);
      } catch (e) {
        log.error('Could not set the country of an app', (e as Error).message);
        return undefined;
      }
    },
    [app],
  );

  const clearAppExit = React.useCallback(
    async (application: string) => {
      try {
        await app.clearAppExit(application);
      } catch (e) {
        log.error('Could not clear the country of an app', (e as Error).message);
      }
    },
    [app],
  );

  const addIncludedApp = React.useCallback(
    async (application: ISplitTunnelingApplication | string) => {
      try {
        await app.addIncludedApp(application);
      } catch (e) {
        log.error('Could not add an app to VPN only for', (e as Error).message);
      }
    },
    [app],
  );

  const removeIncludedApp = React.useCallback(
    async (application: string) => {
      try {
        await app.removeIncludedApp(application);
      } catch (e) {
        log.error('Could not remove an app from VPN only for', (e as Error).message);
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
