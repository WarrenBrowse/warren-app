import React from 'react';
import { useStore } from 'react-redux';

import {
  applyRoutingOps,
  type AppRoute,
  type DefaultRoute,
  planAppLock,
  planAppRoute,
  planDefaultRoute,
  type RoutingOp,
  routingReflects,
} from '../../../../../shared/app-routing';
import { type ISplitTunnelingApplication } from '../../../../../shared/application-types';
import { type AppRoutingSettings } from '../../../../../shared/daemon-rpc-types';
import log from '../../../../../shared/logging';
import { useAppContext } from '../../../../context';
import { type IReduxState } from '../../../../redux/store';

type Subject = { id: string; application: ISplitTunnelingApplication | string };

// How long a change waits for the daemon's settings to show it before the
// next one is planned anyway.
const APPLY_TIMEOUT_MS = 3000;
const POLL_MS = 50;

// Changes run one after the other across every screen of the view: each is
// planned from the settings the daemon reported once the previous one showed
// up in them, never from a snapshot another change is still altering.
let queue: Promise<unknown> = Promise.resolve();

function inTurn<T>(task: () => Promise<T>): Promise<T> {
  const next = queue.then(task);
  queue = next.catch(() => undefined);
  return next;
}

// Applies the daemon calls that give one app a route, or switch the default.
// The calls run in the order the plan chose; a failure stops there, which
// leaves a state the plan already allowed. Failures are logged without their
// message: one can name a path tied to an exit.
export function useRoutingActions() {
  const app = useAppContext();
  const store = useStore<IReduxState>();
  const platform = window.env.platform;

  const execute = React.useCallback(
    async (ops: RoutingOp[], subject?: Subject) => {
      // The app being changed goes by what was picked, so the main process can
      // resolve a program chosen in the file dialog; every other id is stored.
      const named = (id: string) =>
        subject !== undefined && subject.id === id ? subject.application : id;
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
          case 'lock':
            await app.addLockedApp(named(op.app));
            break;
          case 'unlock':
            await app.removeLockedApp(op.app);
            break;
        }
      }
    },
    [app],
  );

  const waitForSettings = React.useCallback(
    async (expected: AppRoutingSettings) => {
      const deadline = Date.now() + APPLY_TIMEOUT_MS;
      while (Date.now() < deadline) {
        if (routingReflects(store.getState().settings.appRouting, expected, platform)) {
          return;
        }
        await new Promise((resolve) => setTimeout(resolve, POLL_MS));
      }
    },
    [platform, store],
  );

  const enqueue = React.useCallback(
    (plan: (routing: AppRoutingSettings) => RoutingOp[], subject?: Subject) => {
      return inTurn(async () => {
        const routing = store.getState().settings.appRouting;
        let ops: RoutingOp[] = [];
        try {
          ops = plan(routing);
          await execute(ops, subject);
        } catch {
          log.error('Could not change the route of an app');
          return false;
        }
        await waitForSettings(applyRoutingOps(routing, ops, platform));
        return true;
      });
    },
    [execute, platform, store, waitForSettings],
  );

  const setAppRoute = React.useCallback(
    (subject: Subject, route: AppRoute) =>
      enqueue((routing) => planAppRoute(routing, subject.id, route, platform), subject),
    [enqueue, platform],
  );

  const setAppLocked = React.useCallback(
    (subject: Subject, locked: boolean) =>
      enqueue((routing) => planAppLock(routing, subject.id, locked, platform), subject),
    [enqueue, platform],
  );

  const setDefaultRoute = React.useCallback(
    (route: DefaultRoute) => enqueue((routing) => planDefaultRoute(routing, route, platform)),
    [enqueue, platform],
  );

  return { setAppRoute, setAppLocked, setDefaultRoute };
}
