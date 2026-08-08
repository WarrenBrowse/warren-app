import { useCallback } from 'react';
import styled from 'styled-components';

import { urls } from '../../../../shared/constants';
import { messages } from '../../../../shared/gettext';
import { useAppContext } from '../../../context';
import { Button, Icon, Spinner } from '../../../lib/components';
import { View } from '../../../lib/components/view';
import { colors, Radius, spacings } from '../../../lib/foundations';
import { useHistory } from '../../../lib/history';
import { useSelector } from '../../../redux/store';
import { AppNavigationHeader } from '../../';
import { BackAction } from '../../keyboard-navigation';
import { NavigationContainer } from '../../NavigationContainer';
import { NavigationScrollbars } from '../../NavigationScrollbars';
import { ForumNotificationCard } from './components';
import { useForumActivity } from './hooks';

// The panel is a stack of cards with breathing room, not a settings list:
// notifications are scanned, not configured.
const StyledStack = styled.div`
  display: flex;
  flex-direction: column;
  gap: ${spacings.tiny};
  padding: 0 ${spacings.medium} ${spacings.medium};
  min-width: 0;
`;

const StyledCentered = styled.div`
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: ${spacings.medium};
  padding: ${spacings.big} ${spacings.medium};
  text-align: center;
`;

const StyledEmptyDisc = styled.div`
  display: flex;
  align-items: center;
  justify-content: center;
  width: 64px;
  height: 64px;
  border-radius: ${Radius.radiusFull};
  background-color: ${colors.blue20};
`;

const StyledMessage = styled.p`
  margin: 0;
  max-width: 32ch;
  color: ${colors.whiteOnDarkBlue60};
  font-size: 13px;
  line-height: 19px;
`;

const StyledHandle = styled.span`
  color: ${colors.whiteOnDarkBlue40};
  font-size: 12px;
`;

/**
 * Community-forum activity, opened from the header bell.
 *
 * A view rather than a popover: Account and Settings both open this way, and
 * a dropdown over the connect screen exists nowhere else in this app.
 */
export function ForumActivityView() {
  const { pop } = useHistory();
  const { openUrl } = useAppContext();
  const handle = useSelector((state) => state.account.forumIdentity?.handle);
  const { state, reload } = useForumActivity();

  const openForum = useCallback(() => void openUrl(urls.forum), [openUrl]);

  // TRANSLATORS: Heading of the community-forum activity panel.
  const title = messages.pgettext('forum-activity-view', 'Forum');

  return (
    <View backgroundColor="darkBlue">
      <BackAction action={pop}>
        <NavigationContainer>
          <AppNavigationHeader title={title} />
          <NavigationScrollbars>
            <View.Content>
              {state.status === 'loading' && (
                <StyledCentered>
                  <Spinner size="big" />
                </StyledCentered>
              )}

              {state.status === 'error' && (
                <StyledCentered>
                  <StyledEmptyDisc>
                    <Icon icon="alert-circle" size="large" color="whiteOnDarkBlue40" />
                  </StyledEmptyDisc>
                  <StyledMessage>
                    {
                      // TRANSLATORS: Shown when the app could not reach the
                      // TRANSLATORS: forum to load the user's notifications.
                      messages.pgettext(
                        'forum-activity-view',
                        'Could not reach the forum. Check your connection and try again.',
                      )
                    }
                  </StyledMessage>
                  <Button onClick={reload}>
                    <Button.Text>
                      {
                        // TRANSLATORS: Button that retries loading the forum
                        // TRANSLATORS: notifications.
                        messages.pgettext('forum-activity-view', 'Try again')
                      }
                    </Button.Text>
                  </Button>
                </StyledCentered>
              )}

              {state.status === 'ready' && state.notifications.length > 0 && (
                <StyledStack>
                  {state.notifications.map((notification) => (
                    <ForumNotificationCard key={notification.id} notification={notification} />
                  ))}
                </StyledStack>
              )}

              {state.status === 'ready' && state.notifications.length === 0 && (
                <StyledCentered>
                  <StyledEmptyDisc>
                    <Icon icon="bell-outline" size="large" color="whiteOnDarkBlue40" />
                  </StyledEmptyDisc>
                  <StyledMessage>
                    {
                      // TRANSLATORS: Shown when the user has no forum
                      // TRANSLATORS: notifications waiting.
                      messages.pgettext(
                        'forum-activity-view',
                        'Nothing new on the forum. Come and ask a question, or help someone else out.',
                      )
                    }
                  </StyledMessage>
                  <Button onClick={openForum}>
                    <Button.Text>
                      {
                        // TRANSLATORS: Button that opens the community forum in
                        // TRANSLATORS: the browser.
                        messages.pgettext('forum-activity-view', 'Open the forum')
                      }
                    </Button.Text>
                  </Button>
                  {handle !== undefined && (
                    <StyledHandle>
                      {
                        // TRANSLATORS: Shown under the empty forum panel, with
                        // TRANSLATORS: the user's own public forum name.
                        // TRANSLATORS: Available placeholder:
                        // TRANSLATORS: %(handle)s - the user's forum name
                        messages
                          .pgettext('forum-activity-view', 'You post as %(handle)s')
                          .replace('%(handle)s', handle)
                      }
                    </StyledHandle>
                  )}
                </StyledCentered>
              )}
            </View.Content>
          </NavigationScrollbars>
        </NavigationContainer>
      </BackAction>
    </View>
  );
}
