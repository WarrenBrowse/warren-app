import { useCallback } from 'react';
import { sprintf } from 'sprintf-js';

import { messages } from '../../../shared/gettext';
import { Button, Text } from '../../lib/components';
import { FlexColumn } from '../../lib/components/flex-column';
import { Link } from '../../lib/components/link';
import { TextField } from '../../lib/components/text-field';

export interface BackupCheckStepProps {
  // Zero-based positions of the asked words, in phrase order.
  positions: number[];
  answers: string[];
  onAnswerChange: (index: number, value: string) => void;
  mismatch: boolean;
  onContinue: () => void;
  onBack: () => void;
}

export function BackupCheckStep({
  positions,
  answers,
  onAnswerChange,
  mismatch,
  onContinue,
  onBack,
}: BackupCheckStepProps) {
  const complete = answers.every((answer) => answer.trim() !== '');

  return (
    <FlexColumn gap="small">
      <Text variant="bodySmall" color="whiteAlpha60">
        {messages.pgettext(
          'login-view',
          'To make sure you saved it, type these words from your recovery phrase.',
        )}
      </Text>

      {positions.map((position, index) => (
        <BackupCheckWordField
          key={position}
          index={index}
          position={position}
          value={answers[index]}
          invalid={mismatch}
          onAnswerChange={onAnswerChange}
        />
      ))}

      {mismatch && (
        <Text variant="bodySmall" color="red" role="alert">
          {messages.pgettext(
            'login-view',
            'These words do not match your recovery phrase. Check what you wrote down.',
          )}
        </Text>
      )}

      <Button
        variant="success"
        onClick={onContinue}
        disabled={!complete}
        data-testid="login-backup-check-continue">
        <Button.Text>{messages.gettext('Continue')}</Button.Text>
      </Button>

      <Link as="button" onClick={onBack} data-testid="login-backup-check-back">
        <Link.Text>{messages.gettext('Back')}</Link.Text>
      </Link>
    </FlexColumn>
  );
}

interface BackupCheckWordFieldProps {
  index: number;
  position: number;
  value: string;
  invalid: boolean;
  onAnswerChange: (index: number, value: string) => void;
}

function BackupCheckWordField({
  index,
  position,
  value,
  invalid,
  onAnswerChange,
}: BackupCheckWordFieldProps) {
  const onValueChange = useCallback(
    (newValue: string) => onAnswerChange(index, newValue),
    [index, onAnswerChange],
  );
  const label = sprintf(
    // TRANSLATORS: Label of a field asking for one word of the recovery phrase.
    // TRANSLATORS: Available placeholders:
    // TRANSLATORS: %(position)d - the place of the word in the phrase, from 1 to 12 or 24.
    messages.pgettext('login-view', 'Word %(position)d'),
    { position: position + 1 },
  );
  return (
    <FlexColumn gap="tiny">
      <Text variant="labelTiny" color="whiteAlpha60">
        {label}
      </Text>
      <TextField value={value} onValueChange={onValueChange} invalid={invalid}>
        <TextField.Input
          aria-label={label}
          autoComplete="off"
          autoCorrect="off"
          autoCapitalize="off"
          spellCheck={false}
          inputMode="text"
          maxLength={16}
          data-testid={`login-backup-check-word-${index}`}
        />
      </TextField>
    </FlexColumn>
  );
}
