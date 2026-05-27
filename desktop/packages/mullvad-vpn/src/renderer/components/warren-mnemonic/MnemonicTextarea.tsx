import React from 'react';
import styled from 'styled-components';

import { messages } from '../../../shared/gettext';
import { Text } from '../../lib/components';
import { FlexColumn } from '../../lib/components/flex-column';
import { colors, Radius, spacings } from '../../lib/foundations';

// 12 or 24 word BIP39 mnemonic input. Shared between the onboarding
// wallet step (`OnboardingWalletView` - import mode) and the settings
// restore view (`RestoreMnemonicView`). Includes a live word counter
// gated on 12 or 24 words for BIP39 validity (the daemon performs the
// real wordlist + checksum validation; the counter is UI feedback
// only).

const StyledTextarea = styled.textarea`
  width: 100%;
  min-height: 110px;
  padding: ${spacings.small};
  background-color: ${colors.darkerBlue50};
  border: 1px solid ${colors.whiteAlpha20};
  border-radius: ${Radius.radius4};
  color: ${colors.white};
  font-family: 'Source Code Pro', Menlo, Consolas, monospace;
  font-size: 14px;
  line-height: 1.5;
  resize: vertical;

  &::placeholder {
    color: ${colors.whiteAlpha40};
  }

  &:focus-visible {
    outline: 2px solid ${colors.white};
    outline-offset: 2px;
  }
`;

export function countMnemonicWords(input: string): number {
  return input.split(/\s+/).filter((w) => w.length > 0).length;
}

export type MnemonicTextareaProps = {
  value: string;
  onValueChange: (value: string) => void;
  placeholder?: string;
  rows?: number;
  'data-testid'?: string;
};

export function MnemonicTextarea({
  value,
  onValueChange,
  placeholder = 'word1 word2 word3 ...',
  rows = 3,
  ...rest
}: MnemonicTextareaProps) {
  const onChange = React.useCallback(
    (e: React.ChangeEvent<HTMLTextAreaElement>) => onValueChange(e.target.value),
    [onValueChange],
  );
  const wordCount = countMnemonicWords(value);

  return (
    <FlexColumn gap="tiny">
      <StyledTextarea
        rows={rows}
        placeholder={placeholder}
        value={value}
        onChange={onChange}
        spellCheck={false}
        autoCapitalize="off"
        autoCorrect="off"
        data-testid={rest['data-testid']}
      />
      <Text variant="labelTiny" color="whiteAlpha60">
        {wordCount}
        {' / 12 '}
        {messages.pgettext('keys-view', 'words')}
      </Text>
    </FlexColumn>
  );
}
