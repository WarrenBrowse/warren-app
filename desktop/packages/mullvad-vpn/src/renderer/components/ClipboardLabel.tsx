import { useCallback } from 'react';
import styled from 'styled-components';

import { messages } from '../../shared/gettext';
import log from '../../shared/logging';
import { useScheduler } from '../../shared/scheduler';
import { Flex, Icon, IconButton } from '../lib/components';
import { useBoolean } from '../lib/utility-hooks';

const COPIED_ICON_DURATION = 2000;

interface IProps extends React.HTMLAttributes<HTMLElement> {
  value: string;
  displayValue?: string;
  message?: string;
}

const StyledLabelContainer = styled.div({
  display: 'flex',
  flex: 1,
  height: '19px',
  alignItems: 'center',
});

const StyledLabel = styled.span({
  flex: 1,
});

export default function ClipboardLabel(props: IProps) {
  const { value, displayValue, message, ...otherProps } = props;

  const [justCopied, setJustCopied, resetJustCopied] = useBoolean(false);

  const copiedScheduler = useScheduler();

  const onCopy = useCallback(async () => {
    try {
      await navigator.clipboard.writeText(value);
      copiedScheduler.schedule(resetJustCopied, COPIED_ICON_DURATION);
      setJustCopied();
    } catch (e) {
      const error = e as Error;
      log.error(`Failed to copy to clipboard: ${error.message}`);
    }
  }, [value, copiedScheduler, setJustCopied, resetJustCopied]);

  return (
    <StyledLabelContainer>
      <StyledLabel {...otherProps}>{displayValue ?? value}</StyledLabel>
      <Flex gap="medium">
        {justCopied ? (
          <Icon icon="checkmark" color="green"></Icon>
        ) : (
          <IconButton
            onClick={onCopy}
            aria-label={
              // TRANSLATORS: Provided to accessibility tools such as screenreaders to describe a button
              // TRANSLATORS: which copies the public key to the clipboard.
              messages.pgettext('accessibility', 'Copy public key')
            }>
            <IconButton.Icon icon={'copy'} />
          </IconButton>
        )}
      </Flex>
    </StyledLabelContainer>
  );
}
