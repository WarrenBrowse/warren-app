import React from 'react';
import { sprintf } from 'sprintf-js';
import styled from 'styled-components';

import { strings } from '../../../../shared/constants';
import { messages } from '../../../../shared/gettext';
import { DaitaSetting } from '../../../features/daita/components';
import { useDaitaDirectOnly, useDaitaUnavailable } from '../../../features/daita/hooks';
import { Flex, Icon, Text } from '../../../lib/components';
import { Carousel } from '../../../lib/components/carousel';
import { FlexColumn } from '../../../lib/components/flex-column';
import { View } from '../../../lib/components/view';
import { colors } from '../../../lib/foundations';
import { useHistory } from '../../../lib/history';
import { AppNavigationHeader } from '../..';
import { BackAction } from '../../keyboard-navigation';
import { NavigationContainer } from '../../NavigationContainer';
import { NavigationScrollbars } from '../../NavigationScrollbars';
import { HeaderTitle } from '../../SettingsHeader';
import { useShowDaitaMultihopInfo } from './hooks';

// Unmissable warning banner: the toggle is on but the server did not grant
// the defense, so the CURRENT connection is not protected by DAITA. Red
// tint + alert icon, sitting above the marketing carousel.
const StyledDaitaUnavailableBanner = styled(Flex)({
  backgroundColor: colors.redAlpha40,
  borderRadius: '8px',
  padding: '12px',
});

export function DaitaSettingsView() {
  const { pop } = useHistory();
  const showDaitaMultihopInfo = useShowDaitaMultihopInfo();
  const daitaUnavailable = useDaitaUnavailable();
  const { daitaDirectOnly, setDaitaDirectOnly } = useDaitaDirectOnly();

  // Warren always uses multihop to enable DAITA, so the "direct only" mode is not
  // exposed. Reset any value persisted by an older build so a user can't get stuck
  // in a state with no UI control to leave it.
  React.useEffect(() => {
    if (daitaDirectOnly) {
      void setDaitaDirectOnly(false);
    }
  }, [daitaDirectOnly, setDaitaDirectOnly]);

  return (
    <View backgroundColor="darkBlue">
      <BackAction action={pop}>
        <NavigationContainer>
          <AppNavigationHeader title={strings.daita} />

          <NavigationScrollbars>
            <View.Content>
              <FlexColumn gap="medium">
                <View.Container horizontalMargin="medium" flexDirection="column" gap="medium">
                  <HeaderTitle>{strings.daita}</HeaderTitle>
                  {daitaUnavailable && (
                    <StyledDaitaUnavailableBanner
                      gap="small"
                      alignItems="center"
                      data-testid="daita-unavailable-banner">
                      <Icon icon="alert-circle" color="red" size="small" />
                      <Text variant="labelTinySemiBold" color="white">
                        {sprintf(
                          messages.pgettext(
                            // TRANSLATORS: Warning banner shown when the user turned DAITA on but the
                            // TRANSLATORS: connected server did not grant the defense, meaning the
                            // TRANSLATORS: current connection is not protected by it.
                            // TRANSLATORS: Available placeholders:
                            // TRANSLATORS: %(daita)s - Will be replaced with DAITA
                            'wireguard-settings-view',
                            '%(daita)s is not active on the current server: this connection is not protected by it. Reconnect or select a different location.',
                          ),
                          { daita: strings.daita },
                        )}
                      </Text>
                    </StyledDaitaUnavailableBanner>
                  )}
                  {showDaitaMultihopInfo && (
                    <Flex gap="small" alignItems="center">
                      <Icon icon="info-circle" color="whiteOnBlue60" size="small" />
                      <Text variant="labelTinySemiBold" color="whiteAlpha60">
                        {messages.pgettext(
                          'wireguard-settings-view',
                          'Multihop is being used to enable DAITA for your selected location',
                        )}
                      </Text>
                    </Flex>
                  )}
                  <FlexColumn gap="large">
                    <Carousel
                      aria-label={
                        // TRANSLATORS: Accessibility label for a carousel that explains the DAITA feature to the user.
                        messages.pgettext('accessibility', 'DAITA information carousel')
                      }>
                      <Carousel.Slides>
                        <Carousel.Slides.Slide>
                          <Carousel.Slides.Slide.Image
                            source="daita-off-illustration"
                            alt={
                              // TRANSLATORS: Alt text for an illustration showing VPN traffic without DAITA.
                              messages.pgettext(
                                'accessibility',
                                'Illustration showing VPN traffic without DAITA',
                              )
                            }
                          />
                          <Carousel.Slides.Slide.TextGroup>
                            <Carousel.Slides.Slide.Text>
                              {sprintf(
                                messages.pgettext(
                                  // TRANSLATORS: Short description of what the DAITA setting does.
                                  // TRANSLATORS: Available placeholders:
                                  // TRANSLATORS: %(daita)s - Will be replaced with DAITA
                                  // TRANSLATORS: %(daitaFull)s - Will be replaced with Defence against AI-guided Traffic Analysis
                                  'wireguard-settings-view',
                                  '%(daita)s (%(daitaFull)s) hides patterns in your encrypted VPN traffic.',
                                ),
                                { daita: strings.daita, daitaFull: strings.daitaFull },
                              )}
                            </Carousel.Slides.Slide.Text>
                            <Carousel.Slides.Slide.Text>
                              {messages.pgettext(
                                // TRANSLATORS: Explains why traffic analysis is a threat even when traffic is encrypted.
                                'wireguard-settings-view',
                                'Even when encrypted, the data packets entering and leaving your device can be analyzed by AI to reveal your activity.',
                              )}
                            </Carousel.Slides.Slide.Text>
                          </Carousel.Slides.Slide.TextGroup>
                        </Carousel.Slides.Slide>
                        <Carousel.Slides.Slide>
                          <Carousel.Slides.Slide.Image
                            source="daita-on-illustration"
                            alt={
                              // TRANSLATORS: Alt text for an illustration showing VPN traffic with DAITA.
                              messages.pgettext(
                                'accessibility',
                                'Illustration showing VPN traffic with DAITA enabled',
                              )
                            }
                          />
                          <Carousel.Slides.Slide.TextGroup>
                            <Carousel.Slides.Slide.Text>
                              {sprintf(
                                messages.pgettext(
                                  // TRANSLATORS: Explains how DAITA protects the user.
                                  // TRANSLATORS: Available placeholders:
                                  // TRANSLATORS: %(daita)s - Will be replaced with DAITA
                                  'wireguard-settings-view',
                                  '%(daita)s adds network noise and makes all packets the same size, so it is much harder to tell which sites you visit or who you communicate with.',
                                ),
                                { daita: strings.daita },
                              )}
                            </Carousel.Slides.Slide.Text>
                            <Carousel.Slides.Slide.Text variant="labelTinySemiBold">
                              {messages.pgettext(
                                // TRANSLATORS: Warning that the DAITA setting increases traffic and affects performance and battery life.
                                'wireguard-settings-view',
                                'Note: this increases network traffic and can reduce speed, latency, and battery life. Use with caution on limited plans.',
                              )}
                            </Carousel.Slides.Slide.Text>
                          </Carousel.Slides.Slide.TextGroup>
                        </Carousel.Slides.Slide>
                      </Carousel.Slides>
                      <Carousel.Controls>
                        <Carousel.Controls.Indicators />
                        <Carousel.Controls.ControlGroup>
                          <Carousel.Controls.PrevButton />
                          <Carousel.Controls.NextButton />
                        </Carousel.Controls.ControlGroup>
                      </Carousel.Controls>
                    </Carousel>
                    <DaitaSetting />
                  </FlexColumn>
                </View.Container>
              </FlexColumn>
            </View.Content>
          </NavigationScrollbars>
        </NavigationContainer>
      </BackAction>
    </View>
  );
}
