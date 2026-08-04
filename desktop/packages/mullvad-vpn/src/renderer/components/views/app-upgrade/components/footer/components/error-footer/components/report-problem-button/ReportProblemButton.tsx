import { useCallback } from 'react';

import { urls } from '../../../../../../../../../../shared/constants';
import { messages } from '../../../../../../../../../../shared/gettext';
import { useAppContext } from '../../../../../../../../../context';
import { Button } from '../../../../../../../../../lib/components';

// The community forum is the support front door: upgrade failures are
// reported there (with logs attached from the user's own bug-report topic),
// not through the removed in-app problem-report form.
export function ReportProblemButton() {
  const { openUrl } = useAppContext();
  const openForum = useCallback(() => openUrl(urls.forum), [openUrl]);

  return (
    <Button onClick={openForum}>
      <Button.Text>
        {
          // TRANSLATORS: Button text to report a problem on the community forum
          messages.pgettext('app-upgrade-view', 'Report a problem on the forum')
        }
      </Button.Text>
    </Button>
  );
}
