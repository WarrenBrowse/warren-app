import { messages } from '../../../../shared/gettext';
import { FlexColumn } from '../../../lib/components/flex-column';
import { View } from '../../../lib/components/view';
import { useHistory } from '../../../lib/history';
import { AppNavigationHeader } from '../..';
import { BackAction } from '../../keyboard-navigation';
import { NavigationContainer } from '../../NavigationContainer';
import { NavigationScrollbars } from '../../NavigationScrollbars';
import { HeaderTitle } from '../../SettingsHeader';
import {
  CommunityButton,
  FaqButton,
  ForumSignInCodeButton,
  RefundPolicyButton,
  ReportProblemButton,
  TermsButton,
} from './components';

export function SupportView() {
  const { pop } = useHistory();

  return (
    <View backgroundColor="darkBlue">
      <BackAction action={pop}>
        <NavigationContainer>
          <AppNavigationHeader
            title={
              // TRANSLATORS: Title label in navigation bar
              messages.pgettext('support-view', 'Support')
            }
          />

          <NavigationScrollbars>
            <View.Content>
              <View.Container horizontalMargin="medium" flexDirection="column" gap="medium">
                <HeaderTitle>{messages.pgettext('support-view', 'Support')}</HeaderTitle>
                <FlexColumn>
                  <CommunityButton />
                  <ForumSignInCodeButton />
                  <ReportProblemButton />
                  <FaqButton />
                  <TermsButton />
                  <RefundPolicyButton />
                </FlexColumn>
              </View.Container>
            </View.Content>
          </NavigationScrollbars>
        </NavigationContainer>
      </BackAction>
    </View>
  );
}
