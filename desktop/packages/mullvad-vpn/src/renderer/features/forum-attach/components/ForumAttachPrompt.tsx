import { useCallback, useEffect, useState } from 'react';
import { sprintf } from 'sprintf-js';
import styled from 'styled-components';

import { ForumAttachResult, IForumAttachRequest } from '../../../../shared/forum-attach';
import { messages } from '../../../../shared/gettext';
import { ModalAlert, ModalAlertType } from '../../../components/Modal';
import { Button } from '../../../lib/components';
import { colors } from '../../../lib/foundations';

// Readable red for the refusal/error notice: the default modal message color
// (whiteAlpha60) is too dim to read on the dark modal background.
const NoticeText = styled.span({
  display: 'block',
  marginTop: '12px',
  color: colors.red,
  fontSize: '13px',
  lineHeight: 1.4,
});

// The renderer reaches IPC through the contextBridge-exposed `window.ipc`,
// never by importing `lib/ipc-event-channel` directly (that module imports
// the `electron` package and crashes the sandboxed renderer on load).

function resultNotice(result: ForumAttachResult): string | undefined {
  switch (result) {
    case 'attached':
      return undefined;
    case 'not-author':
      return messages.pgettext(
        'forum-attach',
        'Only the author of the bug report can attach logs to it.',
      );
    case 'expired':
      return messages.pgettext(
        'forum-attach',
        'This request has expired. Start again from the forum page.',
      );
    case 'too-large':
      return messages.pgettext('forum-attach', 'The log file is too large to attach.');
    case 'error':
      return messages.pgettext(
        'forum-attach',
        'Attaching the logs failed. Please try again in a moment.',
      );
  }
}

// Consent prompt for the community-forum attach-logs flow (doc 55). Mounts a
// modal when a `warren://attach-logs` deep link arrives (via the
// `forumAttach.request` IPC push). The app NEVER sends logs silently: the
// user can view the exact redacted report first, and signing with the wallet
// key happens only after an explicit approval here. Declining notifies the
// connect provider so the waiting browser page unblocks.
export function ForumAttachPrompt() {
  const [request, setRequest] = useState<IForumAttachRequest | undefined>(undefined);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<string | undefined>(undefined);

  useEffect(() => {
    const unsubscribe = window.ipc.forumAttach.listenRequest((req) => {
      setNotice(undefined);
      setBusy(false);
      setRequest(req);
    });
    // A deep link that arrived before this component mounted (cold start,
    // window reopen) was buffered in the main process; a live push racing
    // this fetch wins via the functional update.
    void window.ipc.forumAttach.getPending().then((req) => {
      if (req) {
        setRequest((current) => current ?? req);
      }
    });
    return unsubscribe;
  }, []);

  const close = useCallback(() => {
    setRequest(undefined);
    setBusy(false);
    setNotice(undefined);
  }, []);

  const handleViewLogs = useCallback(() => {
    if (request?.reportId) {
      void window.ipc.problemReport.viewLog(request.reportId);
    }
  }, [request]);

  const handleApprove = useCallback(async () => {
    if (!request) {
      return;
    }
    setBusy(true);
    setNotice(undefined);
    const result = await window.ipc.forumAttach.approve(request);
    setBusy(false);
    if (result === 'attached') {
      close();
    } else {
      setNotice(resultNotice(result));
    }
  }, [request, close]);

  const handleCancel = useCallback(() => {
    if (request) {
      void window.ipc.forumAttach.cancel(request);
    }
    close();
  }, [request, close]);

  return (
    <ModalAlert
      isOpen={request !== undefined}
      type={ModalAlertType.info}
      title={messages.pgettext('forum-attach', 'Attach your logs to the bug report?')}
      message={[
        // Topic 0 is the pre-topic variant: the report is still being
        // composed, so there is no topic number to name yet.
        request?.topicId === 0
          ? messages.pgettext(
              'forum-attach',
              'Your new bug report being composed on the Warren forum asks to attach the app technical logs to help diagnose the problem.',
            )
          : sprintf(
              messages.pgettext(
                'forum-attach',
                'Your bug report on the Warren forum (topic %(topicId)d) asks to attach the app technical logs to help diagnose the problem.',
              ),
              { topicId: request?.topicId ?? 0 },
            ),
        messages.pgettext(
          'forum-attach',
          'The logs are anonymized: account numbers and personal data are redacted. They go privately to the Warren support staff and never appear publicly on the forum. You can view exactly what will be sent before approving.',
        ),
      ]}
      buttons={[
        // No preview button when the deep-link collection failed: approve
        // retries the collection instead.
        ...(request?.reportId
          ? [
              <Button
                key="view"
                disabled={busy}
                onClick={handleViewLogs}
                aria-label={messages.pgettext('forum-attach', 'View the logs')}>
                <Button.Text>{messages.pgettext('forum-attach', 'View the logs')}</Button.Text>
              </Button>,
            ]
          : []),
        <Button
          key="approve"
          variant="success"
          disabled={busy}
          onClick={handleApprove}
          aria-label={messages.pgettext('forum-attach', 'Attach the logs')}>
          <Button.Text>{messages.pgettext('forum-attach', 'Attach the logs')}</Button.Text>
        </Button>,
        <Button
          key="cancel"
          variant="destructive"
          disabled={busy}
          onClick={handleCancel}
          aria-label={messages.pgettext('forum-attach', 'Cancel')}>
          <Button.Text>{messages.pgettext('forum-attach', 'Cancel')}</Button.Text>
        </Button>,
      ]}
      close={handleCancel}>
      {notice && (
        <NoticeText role="alert" aria-live="assertive">
          {notice}
        </NoticeText>
      )}
      <div role="status" aria-live="polite">
        {busy ? messages.pgettext('forum-attach', 'Sending, please wait.') : ''}
      </div>
    </ModalAlert>
  );
}
