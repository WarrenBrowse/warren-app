import { useCallback } from 'react';
import styled from 'styled-components';

import { messages } from '../../../../shared/gettext';
import log from '../../../../shared/logging';
import { useScheduler } from '../../../../shared/scheduler';
import { Flex, FootnoteMini, Icon } from '../../../lib/components';
import { formatWarrenPubKey } from '../../../lib/pubkey';
import { useBoolean } from '../../../lib/utility-hooks';
import { useSelector } from '../../../redux/store';

const COPIED_ICON_DURATION = 2000;

// Reset the native button so the pubkey reads as a plain, tappable label that
// matches the surrounding header typography rather than a chrome button.
const StyledButton = styled.button({
  display: 'inline-flex',
  alignItems: 'center',
  gap: '4px',
  padding: 0,
  border: 'none',
  background: 'transparent',
  cursor: 'pointer',
  color: 'inherit',
  '&:focus-visible': {
    outline: '1px solid var(--color-white-alpha60)',
    outlineOffset: '2px',
    borderRadius: '4px',
  },
});

// Tabular monospace so the SS58 head/tail line up cleanly and never shift the
// adjacent "Time left" label as the address content varies.
const StyledPubKey = styled(FootnoteMini)({
  fontFamily:
    'ui-monospace, SFMono-Regular, "SF Mono", Menlo, Consolas, "Liberation Mono", monospace',
  letterSpacing: '0.02em',
  whiteSpace: 'nowrap',
});

export const AppMainHeaderPubKey = () => {
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
      const error = e as Error;
      log.error(`Failed to copy public key to clipboard: ${error.message}`);
    }
  }, [pubkey, copiedScheduler, resetJustCopied, setJustCopied]);

  if (!pubkey) {
    return null;
  }

  return (
    <Flex alignItems="center">
      <StyledButton
        onClick={onCopy}
        // TRANSLATORS: Provided to accessibility tools such as screenreaders to
        // TRANSLATORS: describe the button which copies the public key to the
        // TRANSLATORS: clipboard.
        aria-label={messages.pgettext('accessibility', 'Copy public key')}>
        <StyledPubKey color={justCopied ? 'whiteAlpha80' : 'whiteAlpha60'}>
          {formatWarrenPubKey(pubkey)}
        </StyledPubKey>
        <Icon
          icon={justCopied ? 'checkmark' : 'copy'}
          size="tiny"
          color={justCopied ? 'green' : 'whiteAlpha40'}
        />
      </StyledButton>
    </Flex>
  );
};
