import { useCallback, useEffect, useState } from 'react';
import styled from 'styled-components';

import { messages } from '../../../../shared/gettext';
import { ModalAlert, ModalAlertType } from '../../../components/Modal';
import { Button } from '../../../lib/components';
import { colors } from '../../../lib/foundations';
import {
  beginForumLoginAttempt,
  bindForumLoginRequest,
  completeForumLoginAttempt,
  forumLoginCodeExpired,
  ForumLoginCompletionState,
  ForumLoginPromptState,
  initialForumLoginPromptState,
  noticeForForumLoginResult,
  revealForumLoginCode,
  settleForumLoginAttempt,
} from '../prompt-state';

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

// The one-time code, large and selectable so it can be read and typed on the
// sign-in page. Never copied for the person: a code on the clipboard is one
// paste away from a chat window.
const CodeText = styled.div({
  display: 'block',
  marginTop: '16px',
  color: colors.white,
  fontFamily: 'ui-monospace, SFMono-Regular, Menlo, Consolas, monospace',
  fontSize: '32px',
  fontWeight: 600,
  letterSpacing: '0.3em',
  textAlign: 'center',
  userSelect: 'text',
});

const WarningText = styled.span({
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
  const [state, setState] = useState<ForumLoginPromptState>(initialForumLoginPromptState);
  const { request, busy, terminal } = state;
  const notice = state.notice === undefined ? undefined : noticeForForumLoginResult(state.notice);

  useEffect(() => {
    const unsubscribe = window.ipc.forumLogin.listenRequest((req) => {
      setState((current) => bindForumLoginRequest(current, req));
    });
    // A deep link that arrived before this component mounted (cold start,
    // window reopen) was buffered in the main process; a live push racing
    // this fetch wins because binding the same sid again changes nothing.
    void window.ipc.forumLogin.getPending().then((req) => {
      if (req) {
        setState((current) => bindForumLoginRequest(current, req));
      }
    });
    return unsubscribe;
  }, []);

  const close = useCallback(() => {
    setState(initialForumLoginPromptState);
  }, []);

  const closeCompletion = useCallback(() => {
    void window.ipc.forumLogin.forgetCompletion();
    close();
  }, [close]);

  const revealCode = useCallback(() => {
    setState((current) => revealForumLoginCode(current));
  }, []);

  const handleApprove = useCallback(async () => {
    if (!request) {
      return;
    }
    setState((current) => beginForumLoginAttempt(current));
    const { result, completion } = await window.ipc.forumLogin.approve(request);
    if (result !== 'approved') {
      setState((current) => settleForumLoginAttempt(current, result));
    } else if (completion) {
      setState((current) => completeForumLoginAttempt(current, completion, Date.now()));
    } else {
      close();
    }
  }, [request, close]);

  // The code completes nothing once its session is dead, so the screen that
  // shows it goes with the session. The wall clock is read every second
  // rather than waited on once: a timer does not count while the machine
  // sleeps, and one that slept past the session would keep the code up.
  const expiresAt = state.completion?.expiresAt;
  useEffect(() => {
    if (expiresAt === undefined) {
      return;
    }
    const timer = setInterval(() => {
      setState((current) =>
        forumLoginCodeExpired(current, Date.now()) ? initialForumLoginPromptState : current,
      );
    }, 1_000);
    return () => clearInterval(timer);
  }, [expiresAt]);

  const handleCancel = useCallback(() => {
    if (request) {
      void window.ipc.forumLogin.cancel(request);
    }
    close();
  }, [request, close]);

  if (state.completion) {
    return (
      <ForumLoginCompletion
        completion={state.completion}
        onReveal={revealCode}
        onClose={closeCompletion}
      />
    );
  }

  // Raised for a QR link and for a code typed under Settings alike, and both
  // are exactly what a relayed sign-in looks like: the browser being signed in
  // is not this app, and nothing on the wire can tell an honest one from an
  // attacker's. Only the person approving can, so the copy states what Warren
  // does not know rather than naming an origin it cannot verify.
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
              'Warren cannot tell which browser is being signed in, or whether it is in front of you right now. Your app will sign a one-time challenge with your wallet key to prove it is you.',
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
          disabled={busy || terminal}
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

interface ForumLoginCompletionProps {
  completion: ForumLoginCompletionState;
  onReveal: () => void;
  onClose: () => void;
}

// The screen of a bound approval (warren-connect docs/FORUM-LOGIN-V2.md): the
// browser that opened the sign-in must present the one-time code. After a
// same-device link main has already opened the handoff page in the default
// browser, and the code waits behind "Show the code" for a sign-in page that
// is in another browser. After a QR or a typed code the code is the screen,
// and after a same-device link the provider answered as a QR's, it comes
// under the warning that the link was relayed.
function ForumLoginCompletion({ completion, onReveal, onClose }: ForumLoginCompletionProps) {
  const [finishing, setFinishing] = useState(false);
  const finishInBrowser = useCallback(async () => {
    setFinishing(true);
    await window.ipc.forumLogin.finishInBrowser();
    setFinishing(false);
  }, []);

  const finishingInBrowser = completion.screen === 'finishing-in-browser';
  const buttons = [];
  if (!completion.codeRevealed) {
    buttons.push(
      <Button key="reveal" onClick={onReveal}>
        <Button.Text>
          {messages.pgettext(
            'forum-login',
            'The sign-in page is in another browser? Show the code',
          )}
        </Button.Text>
      </Button>,
    );
  }
  if (completion.finishInBrowser) {
    buttons.push(
      <Button key="finish" variant="success" disabled={finishing} onClick={finishInBrowser}>
        <Button.Text>
          {messages.pgettext('forum-login', 'Finish in this device’s browser')}
        </Button.Text>
      </Button>,
    );
  }
  buttons.push(
    <Button key="close" onClick={onClose}>
      <Button.Text>{messages.pgettext('forum-login', 'Close')}</Button.Text>
    </Button>,
  );

  return (
    <ModalAlert
      isOpen
      type={ModalAlertType.info}
      title={
        finishingInBrowser
          ? messages.pgettext('forum-login', 'Finishing the sign-in in your browser')
          : messages.pgettext('forum-login', 'Your 6-digit code')
      }
      message={
        finishingInBrowser
          ? messages.pgettext(
              'forum-login',
              'Your browser is finishing the sign-in to the Warren community forum.',
            )
          : completion.screen === 'show-code-relayed'
            ? messages.pgettext(
                'forum-login',
                'This link was for a sign-in on another device, but it did not say so. If someone sent it to you, they are trying to sign in as you: do not give them this code.',
              )
            : undefined
      }
      buttons={buttons}
      close={onClose}>
      {completion.codeRevealed && (
        <>
          <CodeText aria-label={completion.code.split('').join(' ')}>{completion.code}</CodeText>
          <WarningText>
            {messages.pgettext(
              'forum-login',
              'Type this code on the sign-in page of your other device. Never read it out or send it to anyone, including someone who says they are from Warren.',
            )}
          </WarningText>
        </>
      )}
    </ModalAlert>
  );
}
