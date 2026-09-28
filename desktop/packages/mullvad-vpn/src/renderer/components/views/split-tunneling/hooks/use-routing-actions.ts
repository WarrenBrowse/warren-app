import React from 'react';

import {
  type AppRoute,
  type DefaultRoute,
  planAppRoute,
  planDefaultRoute,
  type RoutingOp,
} from '../../../../../shared/app-routing';
import { type ISplitTunnelingApplication } from '../../../../../shared/application-types';
import log from '../../../../../shared/logging';
import { useAppContext } from '../../../../context';
import { useAppRouting } from '../../../../features/app-routing/hooks';

// Applies the daemon calls that give one app a route, or switch the default.
// The calls run one after the other in the order the plan chose; a failure
// stops there, which leaves a state the plan already allowed. Failures are
// logged without their message: one can name a path tied to an exit.
export function useRoutingActions() {
  const app = useAppContext();
  const { routing, platform } = useAppRouting();

  const run = React.useCallback(
    async (
      ops: RoutingOp[],
      subject?: { id: string; application: ISplitTunnelingApplication | string },
    ) => {
      // The app being changed goes by what was picked, so the main process can
      // resolve a program chosen in the file dialog; every other id is known.
      const named = (id: string) =>
        subject !== undefined && subject.id === id ? subject.application : id;
      try {
        for (const op of ops) {
          switch (op.op) {
            case 'set-split-mode':
              await app.setAppSplitMode(op.mode);
              break;
            case 'add-excluded':
              await app.addSplitTunnelingApplication(named(op.app));
              break;
            case 'remove-excluded':
              await app.removeSplitTunnelingApplication(op.app);
              break;
            case 'add-included':
              await app.addIncludedApp(named(op.app));
              break;
            case 'remove-included':
              await app.removeIncludedApp(op.app);
              break;
            case 'set-exit':
              await app.setAppExit(named(op.app), op.exit);
              break;
            case 'clear-exit':
              await app.clearAppExit(op.app);
              break;
            case 'set-exits-enabled':
              await app.setAppExitsEnabled(op.enabled);
              break;
          }
        }
        return true;
      } catch {
        log.error('Could not change the route of an app');
        return false;
      }
    },
    [app],
  );

  const setAppRoute = React.useCallback(
    (subject: { id: string; application: ISplitTunnelingApplication | string }, route: AppRoute) =>
      run(planAppRoute(routing, subject.id, route, platform), subject),
    [platform, routing, run],
  );

  const setDefaultRoute = React.useCallback(
    (route: DefaultRoute) => run(planDefaultRoute(routing, route, platform)),
    [platform, routing, run],
  );

  return { setAppRoute, setDefaultRoute };
}
