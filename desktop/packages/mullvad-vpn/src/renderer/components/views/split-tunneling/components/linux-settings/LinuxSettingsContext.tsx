import React, { useMemo, useState } from 'react';

import { type ILinuxSplitTunnelingApplication } from '../../../../../../shared/application-types';

// `exclude` launches through warren-exclude (Bypass VPN), `include` through
// warren-include (VPN only for).
export type LinuxLaunchMode = 'exclude' | 'include';

type LinuxSettingsContextProviderProps = {
  children: React.ReactNode;
  launchMode: LinuxLaunchMode;
};

type LinuxSettingsContext = {
  launchMode: LinuxLaunchMode;
  applications?: ILinuxSplitTunnelingApplication[];
  browseError?: string;
  searchTerm: string;
  setApplications: (value: ILinuxSplitTunnelingApplication[]) => void;
  setBrowseError: (value?: string) => void;
  setSearchTerm: (value: string) => void;
  setShowUnsupportedDialog: (value: boolean) => void;
  setSplitTunnelingSupported: (value: boolean) => void;
  showUnsupportedDialog: boolean;
  splitTunnelingSupported?: boolean;
};

const LinuxSettingsContext = React.createContext<LinuxSettingsContext | undefined>(undefined);

export const useLinuxSettingsContext = (): LinuxSettingsContext => {
  const context = React.useContext(LinuxSettingsContext);
  if (!context) {
    throw new Error('useLinuxSettingsContext must be used within a LinuxSettingsContext');
  }
  return context;
};

export function LinuxSettingsContextProvider({
  children,
  launchMode,
}: LinuxSettingsContextProviderProps) {
  const [applications, setApplications] = useState<ILinuxSplitTunnelingApplication[]>();
  const [browseError, setBrowseError] = useState<string>();
  const [searchTerm, setSearchTerm] = useState('');
  const [splitTunnelingSupported, setSplitTunnelingSupported] = useState<boolean | undefined>(
    undefined,
  );
  const [showUnsupportedDialog, setShowUnsupportedDialog] = useState(false);

  const value = useMemo(
    () => ({
      launchMode,
      applications,
      browseError,
      searchTerm,
      setApplications,
      setBrowseError,
      setSearchTerm,
      setShowUnsupportedDialog,
      setSplitTunnelingSupported,
      showUnsupportedDialog,
      splitTunnelingSupported,
    }),
    [
      launchMode,
      applications,
      browseError,
      searchTerm,
      setApplications,
      setBrowseError,
      setSearchTerm,
      setShowUnsupportedDialog,
      setSplitTunnelingSupported,
      showUnsupportedDialog,
      splitTunnelingSupported,
    ],
  );

  return <LinuxSettingsContext value={value}>{children}</LinuxSettingsContext>;
}
