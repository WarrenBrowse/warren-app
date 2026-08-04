import React from 'react';

import { useVersionConsistent, useVersionSupported } from '../redux/hooks';
import { useSelector } from '../redux/store';
import { BlockedUpdateView } from './views/blocked-update';

interface BlockingUpdateGateProps {
  children: React.ReactNode;
}

// Hard block for forced updates. Once the daemon reports the running version is
// no longer supported, the entire UI is replaced by the forced-update screen.
// `consistent` guards against acting on a transient GUI/daemon version mismatch
// (which has its own "please restart" notification), and `connectedToDaemon`
// ensures we have real version info rather than the default `supported: true`.
// The tunnel is intentionally left untouched: the user stays protected while
// they update.
export function BlockingUpdateGate({ children }: BlockingUpdateGateProps) {
  const { supported } = useVersionSupported();
  const { consistent } = useVersionConsistent();
  const connectedToDaemon = useSelector((state) => state.userInterface.connectedToDaemon);

  if (connectedToDaemon && consistent && !supported) {
    return <BlockedUpdateView />;
  }

  return <>{children}</>;
}
