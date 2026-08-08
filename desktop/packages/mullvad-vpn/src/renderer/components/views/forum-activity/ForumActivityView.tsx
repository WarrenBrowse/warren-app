import { useCallback } from 'react';

import { urls } from '../../../../shared/constants';
import { messages } from '../../../../shared/gettext';
import { useAppContext } from '../../../context';
import { BodySmall, Button, Flex, TitleBig } from '../../../lib/components';
import { View } from '../../../lib/components/view';
import { useHistory } from '../../../lib/history';
import { useSelector } from '../../../redux/store';
import { AppNavigationHeader } from '../../';
import { BackAction } from '../../keyboard-navigation';
import { NavigationContainer } from '../../NavigationContainer';
import { NavigationScrollbars } from '../../NavigationScrollbars';
import { ForumNotificationRow } from './components';
import { useForumActivity } from './hooks';

/**
 * Community-forum activity, opened from the header bell.
 *
 * A view rather than a popover: Account and Settings both open this way,
 * and a dropdown over the connect screen exists nowhere else in this app.
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
            <View.Content gap="large">
              <View.Container horizontalMargin="medium">
                <TitleBig as="h1">{title}</TitleBig>
              </View.Container>

              {state.status === 'loading' && (
                <View.Container horizontalMargin="medium">
                  <BodySmall color="whiteAlpha60">
                    {
                      // TRANSLATORS: Shown while the app is fetching the
                      // TRANSLATORS: user's forum notifications.
                      messages.pgettext('forum-activity-view', 'Loading your forum activity...')
                    }
                  </BodySmall>
                </View.Container>
              )}

              {state.status === 'error' && (
                <View.Container horizontalMargin="medium" flexDirection="column" gap="medium">
                  <BodySmall color="whiteAlpha60">
                    {
                      // TRANSLATORS: Shown when the app could not reach the
                      // TRANSLATORS: forum to load the user's notifications.
                      messages.pgettext(
                        'forum-activity-view',
                        'Could not reach the forum. Check your connection and try again.',
                      )
                    }
                  </BodySmall>
                  <Button onClick={reload}>
                    <Button.Text>
                      {
                        // TRANSLATORS: Button that retries loading the forum
                        // TRANSLATORS: notifications.
                        messages.pgettext('forum-activity-view', 'Try again')
                      }
                    </Button.Text>
                  </Button>
                </View.Container>
              )}

              {state.status === 'ready' && state.notifications.length > 0 && (
                <Flex flexDirection="column" gap="tiny">
                  <View.Container horizontalMargin="medium" flexDirection="column" gap="tiny">
                    {state.notifications.map((notification) => (
                      <ForumNotificationRow key={notification.id} notification={notification} />
                    ))}
                  </View.Container>
                </Flex>
              )}

              {state.status === 'ready' && state.notifications.length === 0 && (
                <View.Container horizontalMargin="medium" flexDirection="column" gap="medium">
                  {handle !== undefined && (
                    <BodySmall color="whiteAlpha60">
                      {
                        // TRANSLATORS: Shown above the empty forum panel, with
                        // TRANSLATORS: the user's own public forum name.
                        // TRANSLATORS: Available placeholder:
                        // TRANSLATORS: %(handle)s - the user's forum name
                        messages
                          .pgettext('forum-activity-view', 'You post as %(handle)s')
                          .replace('%(handle)s', handle)
                      }
                    </BodySmall>
                  )}
                  <BodySmall color="whiteAlpha60">
                    {
                      // TRANSLATORS: Shown when the user has no forum
                      // TRANSLATORS: notifications waiting.
                      messages.pgettext(
                        'forum-activity-view',
                        'Nothing new on the forum. Come and ask a question, or help someone else out.',
                      )
                    }
                  </BodySmall>
                  <Button onClick={openForum}>
                    <Button.Text>
                      {
                        // TRANSLATORS: Button that opens the community forum in
                        // TRANSLATORS: the browser.
                        messages.pgettext('forum-activity-view', 'Open the forum')
                      }
                    </Button.Text>
                  </Button>
                </View.Container>
              )}
            </View.Content>
          </NavigationScrollbars>
        </NavigationContainer>
      </BackAction>
    </View>
  );
}
