import styled from 'styled-components';

import { messages } from '../../../../shared/gettext';
import { Flex } from '../../../lib/components';
import { FlexColumn } from '../../../lib/components/flex-column';
import { View } from '../../../lib/components/view';
import { colors } from '../../../lib/foundations';
import { IconBadge } from '../../../lib/icon-badge';
import { AppMainHeader } from '../../app-main-header';
import { bigText, measurements, smallText } from '../../common-styles';
import CustomScrollbars from '../../CustomScrollbars';
import { Footer } from '../app-upgrade/components';
import { QuitButton } from '../settings/components/quit-button';

const StyledCustomScrollbars = styled(CustomScrollbars)({
  flex: 1,
});

const StyledTitle = styled.span(bigText, {
  lineHeight: '38px',
  marginBottom: '8px',
  color: colors.white,
});

const StyledMessage = styled.span(smallText, {
  marginBottom: measurements.rowVerticalMargin,
  color: colors.white,
});

// Forced-update screen shown when the daemon reports the running version is no
// longer supported. It replaces the whole UI so the app cannot be used until
// it is updated. The download/verify/install machinery is the same `Footer`
// the voluntary "Update available" view uses; on Linux (no in-app installer)
// that footer falls back to a manual download link. The header carries no
// settings or account button so there is no way out except updating or
// quitting.
export function BlockedUpdateView() {
  return (
    <View backgroundColor="darkBlue">
      <AppMainHeader logoVariant="both" />
      <StyledCustomScrollbars fillContainer>
        <View.Content>
          <View.Container
            flexDirection="column"
            horizontalMargin="large"
            justifyContent="space-between"
            flexGrow={1}
            margin={{ top: 'large' }}>
            <FlexColumn>
              <Flex justifyContent="center" margin={{ bottom: 'medium' }}>
                <IconBadge state="negative" />
              </Flex>
              <StyledTitle data-testid="title">
                {
                  // TRANSLATORS: Title of the screen that forces the user to update.
                  messages.pgettext('app-upgrade-view', 'Update required')
                }
              </StyledTitle>
              <StyledMessage>
                {messages.pgettext(
                  'app-upgrade-view',
                  'This version of Warren is no longer supported. To keep your connection protected, you must update the app to keep using it.',
                )}
              </StyledMessage>
            </FlexColumn>
            <FlexColumn gap="medium">
              <Footer />
              <QuitButton />
            </FlexColumn>
          </View.Container>
        </View.Content>
      </StyledCustomScrollbars>
    </View>
  );
}
