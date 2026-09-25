import React, { useCallback, useMemo, useState } from 'react';

import { modeChangeConfirmation } from '../../../../shared/app-routing';
import { AppSplitMode } from '../../../../shared/daemon-rpc-types';
import { AppRoutingTab } from '../../../../shared/ipc-types';
import { useAppRouting } from '../../../features/app-routing/hooks';
import { useHistory } from '../../../lib/history';
import { useStyledRef } from '../../../lib/utility-hooks';
import { type CustomScrollbarsRef } from '../../CustomScrollbars';

type SplitTunnelingContextProviderProps = {
  children: React.ReactNode;
};

type SplitTunnelingContext = {
  browsing: boolean;
  scrollbarsRef: React.RefObject<CustomScrollbarsRef | null>;
  setBrowsing: (value: boolean) => void;
  tab: AppRoutingTab;
  setTab: (tab: AppRoutingTab) => void;
  // A split mode waiting for the user to confirm what it changes.
  pendingSplitMode?: AppSplitMode;
  requestSplitMode: (mode: AppSplitMode) => void;
  confirmSplitMode: () => void;
  cancelSplitMode: () => void;
};

const SplitTunnelingContext = React.createContext<SplitTunnelingContext | undefined>(undefined);

export const useSplitTunnelingContext = (): SplitTunnelingContext => {
  const context = React.useContext(SplitTunnelingContext);
  if (!context) {
    throw new Error('useSplitTunnelingContext must be used within a SplitTunnelingContext');
  }
  return context;
};

// A link can name the tab to open; otherwise the view opens on the mode in
// force, so a user who turned on "VPN only for" finds it where they left it.
function useInitialTab(splitMode: AppSplitMode): AppRoutingTab {
  const { location } = useHistory();
  const requested = location.state?.options?.find((option) => option.type === 'app-routing-tab');
  if (requested) {
    return requested.tab;
  }
  return splitMode === 'include-only' ? 'include-only' : 'bypass';
}

export function SplitTunnelingContextProvider({ children }: SplitTunnelingContextProviderProps) {
  const [browsing, setBrowsing] = useState(false);
  const scrollbarsRef = useStyledRef<CustomScrollbarsRef>();
  const { routing, setSplitMode } = useAppRouting();
  const initialTab = useInitialTab(routing.splitMode);
  const [tab, setTab] = useState<AppRoutingTab>(initialTab);
  const [pendingSplitMode, setPendingSplitMode] = useState<AppSplitMode>();

  const requestSplitMode = useCallback(
    (mode: AppSplitMode) => {
      if (modeChangeConfirmation(routing.splitMode, mode)) {
        setPendingSplitMode(mode);
      } else {
        void setSplitMode(mode);
      }
    },
    [routing.splitMode, setSplitMode],
  );

  const confirmSplitMode = useCallback(() => {
    if (pendingSplitMode !== undefined) {
      void setSplitMode(pendingSplitMode);
    }
    setPendingSplitMode(undefined);
  }, [pendingSplitMode, setSplitMode]);

  const cancelSplitMode = useCallback(() => setPendingSplitMode(undefined), []);

  const value = useMemo(
    () => ({
      browsing,
      scrollbarsRef,
      setBrowsing,
      tab,
      setTab,
      pendingSplitMode,
      requestSplitMode,
      confirmSplitMode,
      cancelSplitMode,
    }),
    [
      browsing,
      scrollbarsRef,
      tab,
      pendingSplitMode,
      requestSplitMode,
      confirmSplitMode,
      cancelSplitMode,
    ],
  );

  return <SplitTunnelingContext value={value}>{children}</SplitTunnelingContext>;
}
