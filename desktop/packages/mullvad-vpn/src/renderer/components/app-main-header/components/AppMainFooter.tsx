import { useCallback, useEffect, useState } from 'react';
import styled from 'styled-components';

import { closeToExpiry, formatRemainingTime, hasExpired } from '../../../../shared/account-expiry';
import { messages } from '../../../../shared/gettext';
import log from '../../../../shared/logging';
import { useScheduler } from '../../../../shared/scheduler';
import { Icon } from '../../../lib/components';
import { FontFamilies, homeTokens, surfaces } from '../../../lib/foundations';
import { useInterval } from '../../../lib/hooks';
import { formatWarrenPubKey } from '../../../lib/pubkey';
import { useBoolean } from '../../../lib/utility-hooks';
import { useSelector } from '../../../redux/store';

const { footer } = homeTokens;
const COPIED_ICON_DURATION = 2000;

const StyledFooter = styled.footer`
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: ${footer.gap}px;
  padding: ${footer.paddingVertical}px ${footer.paddingHorizontal}px;
  background-color: ${surfaces.homeFooter};
  color: ${surfaces.homeTextSecondary};
  font-family: ${FontFamilies.sans};
  font-size: ${footer.textSize}px;
  line-height: 16px;
`;

const StyledCopyButton = styled.button`
  display: inline-flex;
  align-items: center;
  gap: ${footer.gap}px;
  padding: 0;
  border: none;
  background: transparent;
  color: inherit;
  font: inherit;
  font-family: ${FontFamilies.mono};
  white-space: nowrap;
  cursor: pointer;
  &:hover {
    color: ${surfaces.homeText};
  }
  &:focus-visible {
    outline: 1px solid ${surfaces.homeText};
    outline-offset: 2px;
    border-radius: 4px;
  }
`;

const StyledCopyIcon = styled(Icon)<{ $copied: boolean }>`
  && {
    width: ${footer.icon}px;
    height: ${footer.icon}px;
    background-color: ${(props) => (props.$copied ? surfaces.homePillProtected : 'currentColor')};
  }
`;

const StyledTimeLeft = styled.span`
  margin-inline-start: auto;
  white-space: nowrap;
`;

function FooterPubKey() {
  const pubkey = useSelector((state) => state.account.pubkey);
  const [justCopied, setJustCopied, resetJustCopied] = useBoolean(false);
  const copiedScheduler = useScheduler();

  const onCopy = useCallback(async () => {
    if (!pubkey) {
      return;
    }
    try {
      await navigator.clipboard.writeText(pubkey);
      copiedScheduler.schedule(resetJustCopied, COPIED_ICON_DURATION);
      setJustCopied();
    } catch (e) {
      log.error(`Failed to copy public key to clipboard: ${(e as Error).message}`);
    }
  }, [pubkey, copiedScheduler, resetJustCopied, setJustCopied]);

  if (!pubkey) {
    return null;
  }

  return (
    <StyledCopyButton
      onClick={onCopy}
      // TRANSLATORS: Provided to accessibility tools such as screenreaders to
      // TRANSLATORS: describe the button which copies the public key to the
      // TRANSLATORS: clipboard.
      aria-label={messages.pgettext('accessibility', 'Copy public key')}>
      {/* An address reads left to right in every language; without this, the digits on either
          side of the ellipsis swap places in Arabic and Persian. */}
      <span dir="ltr">{formatWarrenPubKey(pubkey)}</span>
      <StyledCopyIcon icon={justCopied ? 'checkmark' : 'copy'} $copied={justCopied} />
    </StyledCopyButton>
  );
}

function FooterTimeLeft() {
  const expiry = useSelector((state) => state.account.expiry);
  const [timeLeft, setTimeLeft] = useState(formatTimeLeft(expiry));

  useInterval(() => setTimeLeft(formatTimeLeft(expiry)), 60 * 60 * 1_000);
  useEffect(() => setTimeLeft(formatTimeLeft(expiry)), [expiry]);

  // Close to expiry, the notification area takes over with the renewal call.
  if (!expiry || closeToExpiry(expiry) || hasExpired(expiry)) {
    return null;
  }
  return <StyledTimeLeft data-testid="time-left">{timeLeft}</StyledTimeLeft>;
}

function formatTimeLeft(expiry?: string): string {
  return expiry ? formatRemainingTime(expiry, { suffix: true }) : '';
}

export function AppMainFooter() {
  return (
    <StyledFooter>
      <FooterPubKey />
      <FooterTimeLeft />
    </StyledFooter>
  );
}
