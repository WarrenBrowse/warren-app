import React from 'react';

import { type ISplitTunnelingApplication } from '../../../../../shared/application-types';
import { useAppContext } from '../../../../context';
import { useAfterTransition } from '../../../../lib/transition-hooks';

// The apps a per-app country or the include-only list can name, loaded once
// the view's transition has finished (scanning is slow on a cold cache) and
// refreshed behind a cached answer.
export function useRoutingApplications() {
  const { getAppRoutingApplications } = useAppContext();
  const runAfterTransition = useAfterTransition();
  const [applications, setApplications] = React.useState<ISplitTunnelingApplication[]>();

  const reload = React.useCallback(async () => {
    const { applications } = await getAppRoutingApplications(false);
    setApplications(applications);
  }, [getAppRoutingApplications]);

  React.useEffect(() => {
    let cancelled = false;
    runAfterTransition(async () => {
      const first = await getAppRoutingApplications(false);
      if (cancelled) return;
      setApplications(first.applications);
      if (first.fromCache) {
        const fresh = await getAppRoutingApplications(true);
        if (!cancelled) setApplications(fresh.applications);
      }
    });
    return () => {
      cancelled = true;
    };
  }, [getAppRoutingApplications, runAfterTransition]);

  return { applications, reload };
}
