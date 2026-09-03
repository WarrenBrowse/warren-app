import { messages } from '../../../../../../shared/gettext';
import { RoutePath } from '../../../../../../shared/routes';
import { SettingsNavigationListItem } from '../../../../settings-navigation-list-item';

// The way into the in-app bug report (doc 55): the forum's "Report a bug"
// form filed from the app with the wallet signature and the redacted logs, so
// a person who cannot get through the browser still reaches the support team.
export function ReportProblemButton() {
  return (
    <SettingsNavigationListItem to={RoutePath.reportProblem}>
      <SettingsNavigationListItem.Item>
        <SettingsNavigationListItem.Item.Label>
          {
            // TRANSLATORS: Navigation row to the in-app bug report form.
            messages.pgettext('report-problem', 'Report a problem')
          }
        </SettingsNavigationListItem.Item.Label>
        <SettingsNavigationListItem.Item.ActionGroup>
          <SettingsNavigationListItem.Item.Icon icon="chevron-right" />
        </SettingsNavigationListItem.Item.ActionGroup>
      </SettingsNavigationListItem.Item>
      <SettingsNavigationListItem.Footer>
        <SettingsNavigationListItem.Footer.Text>
          {
            // TRANSLATORS: Subtitle of that row.
            messages.pgettext(
              'report-problem',
              'Send a description and your logs to the support team',
            )
          }
        </SettingsNavigationListItem.Footer.Text>
      </SettingsNavigationListItem.Footer>
    </SettingsNavigationListItem>
  );
}
