import { useCallback, useEffect, useState } from 'react';
import styled from 'styled-components';

import { IForumLoginRequest } from '../../../../shared/forum-login';
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

// The renderer reaches IPC through the contextBridge-exposed `window.ipc`, never
// by importing `lib/ipc-event-channel` directly: that module imports the
// `electron` package, which Vite pre-bundles into the (sandboxed, node-less)
// renderer and crashes it on load with `__dirname is not defined`.

// Consent prompt for the community-forum wallet login (doc 55). Mounts a modal
// when a `warren://forum-login` deep link arrives (via the `forumLogin.request`
// IPC push). The app NEVER logs into the forum silently: signing with the
// wallet key happens only after the user explicitly approves here. Declining
// notifies the connect provider so the waiting browser page unblocks.
export function ForumLoginPrompt() {
  const [request, setRequest] = useState<IForumLoginRequest | undefined>(undefined);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<string | undefined>(undefined);

  useEffect(() => {
    return window.ipc.forumLogin.listenRequest((req) => {
      setNotice(undefined);
      setBusy(false);
      setRequest(req);
    });
  }, []);

  const close = useCallback(() => {
    setRequest(undefined);
    setBusy(false);
    setNotice(undefined);
  }, []);

  const handleApprove = useCallback(async () => {
    if (!request) {
      return;
    }
    setBusy(true);
    setNotice(undefined);
    const result = await window.ipc.forumLogin.approve(request);
    setBusy(false);
    if (result === 'approved') {
      close();
    } else if (result === 'subscription-required') {
      setNotice(
        messages.pgettext(
          'forum-login',
          'Forum access requires a Warren subscription. This wallet has never subscribed.',
        ),
      );
    } else {
      setNotice(messages.pgettext('forum-login', 'Sign-in failed. Please try again in a moment.'));
    }
  }, [request, close]);

  const handleCancel = useCallback(() => {
    if (request) {
      void window.ipc.forumLogin.cancel(request);
    }
    close();
  }, [request, close]);

  return (
    <ModalAlert
      isOpen={request !== undefined}
      type={ModalAlertType.info}
      title={messages.pgettext('forum-login', 'Sign in to the Warren community forum?')}
      message={[
        messages.pgettext(
          'forum-login',
          'A sign-in to the Warren community forum was requested. Your app will sign a one-time challenge with your wallet key to prove it is you.',
        ),
        messages.pgettext(
          'forum-login',
          'No email and no password are used. You appear under an anonymous handle that cannot be linked to your Warren account. Only approve if you started this sign-in.',
        ),
      ]}
      buttons={[
        <Button
          key="approve"
          variant="success"
          disabled={busy}
          onClick={handleApprove}
          aria-label={messages.pgettext('forum-login', 'Approve sign-in')}>
          <Button.Text>{messages.pgettext('forum-login', 'Approve sign-in')}</Button.Text>
        </Button>,
        <Button
          key="cancel"
          variant="destructive"
          disabled={busy}
          onClick={handleCancel}
          aria-label={messages.pgettext('forum-login', 'Cancel')}>
          <Button.Text>{messages.pgettext('forum-login', 'Cancel')}</Button.Text>
        </Button>,
      ]}
      close={handleCancel}>
      {notice && (
        <NoticeText role="alert" aria-live="assertive">
          {notice}
        </NoticeText>
      )}
      <div role="status" aria-live="polite">
        {busy ? messages.pgettext('forum-login', 'Signing, please wait.') : ''}
      </div>
    </ModalAlert>
  );
}
