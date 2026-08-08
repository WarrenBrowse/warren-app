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

// The global theme sets a background color but no default text color, so a bare
// element inherits the browser default (black) and is invisible on the dark
// modal. The transient "Signing" status must set its own readable color.
const StatusText = styled.div({
  display: 'block',
  marginTop: '12px',
  color: colors.whiteAlpha60,
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
    const unsubscribe = window.ipc.forumLogin.listenRequest((req) => {
      setNotice(undefined);
      setBusy(false);
      setRequest(req);
    });
    // A deep link that arrived before this component mounted (cold start,
    // window reopen) was buffered in the main process; a live push racing
    // this fetch wins via the functional update.
    void window.ipc.forumLogin.getPending().then((req) => {
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

  // A cross-device link (the QR on the approval page) is also exactly what a
  // relayed sign-in looks like: the browser being signed in is not this
  // machine. Nothing on the wire can tell those apart, only the person
  // approving can, so the prompt names the situation instead of asking the
  // same neutral question either way.
  const crossDevice = request?.crossDevice === true;

  return (
    <ModalAlert
      isOpen={request !== undefined}
      type={crossDevice ? ModalAlertType.warning : ModalAlertType.info}
      title={
        crossDevice
          ? messages.pgettext('forum-login', 'Sign in to the forum on another device?')
          : messages.pgettext('forum-login', 'Sign in to the Warren community forum?')
      }
      message={[
        crossDevice
          ? messages.pgettext(
              'forum-login',
              'This request came from a QR code, so the browser being signed in is on another device, not this one. Your app will sign a one-time challenge with your wallet key to prove it is you.',
            )
          : messages.pgettext(
              'forum-login',
              'A sign-in to the Warren community forum was requested. Your app will sign a one-time challenge with your wallet key to prove it is you.',
            ),
        crossDevice
          ? messages.pgettext(
              'forum-login',
              'Approve only if you are looking at that sign-in page right now. If someone sent you this code, they are signing in as you. No email and no password are used.',
            )
          : messages.pgettext(
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
      <StatusText role="status" aria-live="polite">
        {busy ? messages.pgettext('forum-login', 'Signing, please wait.') : ''}
      </StatusText>
    </ModalAlert>
  );
}
