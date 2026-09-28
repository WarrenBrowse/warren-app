import React, { useCallback, useMemo, useState } from 'react';

import { type ISplitTunnelingApplication } from '../../../../shared/application-types';
import { useStyledRef } from '../../../lib/utility-hooks';
import { type CustomScrollbarsRef } from '../../CustomScrollbars';
import { useRoutingApplications } from './hooks/use-routing-applications';

// The app a route or country screen is about: one from the list, or a program
// the user picked, which the main process resolves to the id the daemon keys.
export type RoutingTarget = {
  id: string;
  name: string;
  application: ISplitTunnelingApplication | string;
};

export type RoutingScreen = 'list' | 'add' | 'route' | 'country';

type SplitTunnelingContextProviderProps = {
  children: React.ReactNode;
};

type SplitTunnelingContext = {
  browsing: boolean;
  scrollbarsRef: React.RefObject<CustomScrollbarsRef | null>;
  setBrowsing: (value: boolean) => void;
  screen: RoutingScreen;
  target?: RoutingTarget;
  showList: () => void;
  showAdd: () => void;
  showRoute: (target?: RoutingTarget) => void;
  showCountry: () => void;
  // Every app a rule can name, undefined while the first scan runs.
  catalog?: ISplitTunnelingApplication[];
  reloadCatalog: () => Promise<void>;
};

const SplitTunnelingContext = React.createContext<SplitTunnelingContext | undefined>(undefined);

export const useSplitTunnelingContext = (): SplitTunnelingContext => {
  const context = React.useContext(SplitTunnelingContext);
  if (!context) {
    throw new Error('useSplitTunnelingContext must be used within a SplitTunnelingContext');
  }
  return context;
};

export function SplitTunnelingContextProvider({ children }: SplitTunnelingContextProviderProps) {
  const [browsing, setBrowsing] = useState(false);
  const scrollbarsRef = useStyledRef<CustomScrollbarsRef>();
  const [screen, setScreen] = useState<RoutingScreen>('list');
  const [target, setTarget] = useState<RoutingTarget>();
  const { applications: catalog, reload: reloadCatalog } = useRoutingApplications();

  const showList = useCallback(() => {
    setScreen('list');
    setTarget(undefined);
  }, []);
  const showAdd = useCallback(() => setScreen('add'), []);
  const showRoute = useCallback((next?: RoutingTarget) => {
    if (next !== undefined) {
      setTarget(next);
    }
    setScreen('route');
  }, []);
  const showCountry = useCallback(() => setScreen('country'), []);

  const value = useMemo(
    () => ({
      browsing,
      scrollbarsRef,
      setBrowsing,
      screen,
      target,
      showList,
      showAdd,
      showRoute,
      showCountry,
      catalog,
      reloadCatalog,
    }),
    [
      browsing,
      scrollbarsRef,
      screen,
      target,
      showList,
      showAdd,
      showRoute,
      showCountry,
      catalog,
      reloadCatalog,
    ],
  );

  return <SplitTunnelingContext value={value}>{children}</SplitTunnelingContext>;
}
