import { messages } from '../../../../../../shared/gettext';
import { RoutePath } from '../../../../../../shared/routes';
import { SettingsNavigationListItem } from '../../../../settings-navigation-list-item';

// The browser-independent way into the forum sign-in: the approval page shows
// its session id as a code when its button did not open the app, and this row
// is where that code goes. Sits right under the forum link so the two read as
// one path with a fallback.
export function ForumSignInCodeButton() {
  return (
    <SettingsNavigationListItem to={RoutePath.forumSignInCode}>
      <SettingsNavigationListItem.Item>
        <SettingsNavigationListItem.Item.Label>
          {
            // TRANSLATORS: Navigation row to the view where a forum sign-in
            // TRANSLATORS: code, shown on the forum's approval page, is typed.
            messages.pgettext('forum-sign-in-code', 'Sign in to the forum with a code')
          }
        </SettingsNavigationListItem.Item.Label>
        <SettingsNavigationListItem.Item.ActionGroup>
          <SettingsNavigationListItem.Item.Icon icon="chevron-right" />
        </SettingsNavigationListItem.Item.ActionGroup>
      </SettingsNavigationListItem.Item>
      <SettingsNavigationListItem.Footer>
        <SettingsNavigationListItem.Footer.Text>
          {
            // TRANSLATORS: Subtitle of that row: when the fallback is for.
            messages.pgettext(
              'forum-sign-in-code',
              'When the forum\'s "Open the Warren app" button did nothing',
            )
          }
        </SettingsNavigationListItem.Footer.Text>
      </SettingsNavigationListItem.Footer>
    </SettingsNavigationListItem>
  );
}
