import React, { useCallback, useState } from 'react';
import styled from 'styled-components';

import { normalizeForumSignInCode } from '../../../../shared/forum-login';
import { messages } from '../../../../shared/gettext';
import { Button } from '../../../lib/components';
import { TextField } from '../../../lib/components/text-field';
import { View } from '../../../lib/components/view';
import { colors, spacings } from '../../../lib/foundations';
import { useHistory } from '../../../lib/history';
import { AppNavigationHeader } from '../..';
import { BackAction } from '../../keyboard-navigation';
import { NavigationContainer } from '../../NavigationContainer';
import { NavigationScrollbars } from '../../NavigationScrollbars';

const StyledForm = styled.form`
  display: flex;
  flex-direction: column;
  gap: ${spacings.medium};
  padding: 0 ${spacings.medium} ${spacings.medium};
`;

const StyledIntro = styled.p`
  margin: 0;
  color: ${colors.whiteOnDarkBlue60};
  font-size: 13px;
  line-height: 19px;
`;

const StyledLabel = styled.label`
  display: flex;
  flex-direction: column;
  gap: ${spacings.tiny};
  color: ${colors.white};
  font-size: 13px;
  line-height: 19px;
`;

// The same readable red as the consent prompt's refusal notice: the default
// dim text is unreadable on the dark view background.
const StyledInvalid = styled.span`
  color: ${colors.red};
  font-size: 13px;
  line-height: 19px;
`;

/**
 * The forum sign-in finished by hand: the approval page shows its session id
 * as a code when clicking its button did not open the app (a browser that
 * asks first, no handler registered, an old install). Typing it here raises
 * the very same consent prompt a deep link would, so the browser stops being
 * a single point of failure between the forum and the wallet.
 */
export function ForumSignInCodeView() {
  const { pop } = useHistory();
  const inputRef = React.useRef<HTMLInputElement>(null);
  const invalidId = React.useId();
  const [value, setValue] = useState('');
  const [invalid, setInvalid] = useState(false);

  const handleValueChange = useCallback((next: string) => {
    setValue(next);
    setInvalid(false);
  }, []);

  const submit = useCallback(
    async (event: React.FormEvent) => {
      event.preventDefault();
      // Checked here for the inline message; main checks again as the
      // boundary that decides what a code stands for.
      if (normalizeForumSignInCode(value) === undefined) {
        setInvalid(true);
        inputRef.current?.focus();
        return;
      }
      const accepted = await window.ipc.forumLogin.requestFromCode(value);
      if (accepted) {
        // The consent prompt is mounted over every view; leaving this one
        // puts it over the support page the person came from.
        pop();
      } else {
        setInvalid(true);
      }
    },
    [pop, value],
  );

  // TRANSLATORS: Title of the view where a forum sign-in code is typed.
  const title = messages.pgettext('forum-sign-in-code', 'Sign in to the forum with a code');

  return (
    <View backgroundColor="darkBlue">
      <BackAction action={pop}>
        <NavigationContainer>
          <AppNavigationHeader title={title} />
          <NavigationScrollbars>
            <View.Content>
              <StyledForm onSubmit={submit}>
                <StyledIntro>
                  {
                    // TRANSLATORS: Explains where the code comes from and what
                    // TRANSLATORS: typing it does.
                    messages.pgettext(
                      'forum-sign-in-code',
                      'On the forum sign-in page, the session code is shown under the button. Type it here to approve that sign-in from this app. The page keeps waiting while you do.',
                    )
                  }
                </StyledIntro>
                <StyledLabel>
                  {
                    // TRANSLATORS: Label of the sign-in code input field.
                    messages.pgettext('forum-sign-in-code', 'Sign-in code')
                  }
                  <TextField value={value} onValueChange={handleValueChange} invalid={invalid}>
                    <TextField.Input
                      ref={inputRef}
                      placeholder="0123456789abcdef0123456789abcdef"
                      autoComplete="off"
                      autoCorrect="off"
                      autoCapitalize="off"
                      spellCheck={false}
                      inputMode="text"
                      maxLength={64}
                      aria-describedby={invalid ? invalidId : undefined}
                    />
                  </TextField>
                </StyledLabel>
                {invalid && (
                  <StyledInvalid id={invalidId} role="alert" aria-live="assertive">
                    {
                      // TRANSLATORS: Shown when the typed code is not a
                      // TRANSLATORS: 32-character session id.
                      messages.pgettext(
                        'forum-sign-in-code',
                        'A sign-in code is 32 letters and digits, as shown on the forum page.',
                      )
                    }
                  </StyledInvalid>
                )}
                <Button type="submit" disabled={value.trim() === ''}>
                  <Button.Text>
                    {
                      // TRANSLATORS: Button that hands the typed code to the
                      // TRANSLATORS: consent prompt.
                      messages.pgettext('forum-sign-in-code', 'Continue')
                    }
                  </Button.Text>
                </Button>
              </StyledForm>
            </View.Content>
          </NavigationScrollbars>
        </NavigationContainer>
      </BackAction>
    </View>
  );
}
