import { useCallback } from 'react';

import { useDns } from './use-dns';

export function useSetAllowExternalDns() {
  const { dns, setDns } = useDns();

  return useCallback(
    (enabled: boolean) => setDns({ ...dns, allowExternalDns: enabled }),
    [setDns, dns],
  );
}
