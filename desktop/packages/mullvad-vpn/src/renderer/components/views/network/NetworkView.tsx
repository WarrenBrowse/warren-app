import styled from 'styled-components';

import { messages } from '../../../../shared/gettext';
import { Spinner } from '../../../lib/components';
import { View } from '../../../lib/components/view';
import { colors, Radius, spacings } from '../../../lib/foundations';
import { useHistory } from '../../../lib/history';
import { liveThresholdNote, useWarrenNetworkStats } from '../../../lib/network-stats';
import { AppNavigationHeader } from '../../';
import { BackAction } from '../../keyboard-navigation';
import { NavigationContainer } from '../../NavigationContainer';
import { NavigationScrollbars } from '../../NavigationScrollbars';
import { InfoGlyph } from '../../network-stats';
import { ExitCard, FleetCard, Methodology } from './components';
import { useExitPlaces } from './hooks';

const StyledStack = styled.div({
  display: 'flex',
  flexDirection: 'column',
  gap: spacings.small,
  padding: `0 ${spacings.medium} ${spacings.large}`,
  minWidth: 0,
});

const StyledSectionTitle = styled.h2({
  margin: `${spacings.medium} 0 0`,
  color: colors.white,
  fontFamily: 'var(--font-family-open-sans)',
  fontSize: '14px',
  fontWeight: 600,
  lineHeight: '20px',
});

const StyledNote = styled.p({
  display: 'flex',
  alignItems: 'flex-start',
  gap: spacings.small,
  margin: 0,
  padding: `${spacings.small} 12px`,
  borderRadius: Radius.radius12,
  backgroundColor: colors.blue10,
  color: colors.whiteAlpha80,
  fontFamily: 'var(--font-family-open-sans)',
  fontSize: '12px',
  lineHeight: '18px',
  '& > svg': {
    flexShrink: 0,
    marginTop: '3px',
    color: colors.whiteAlpha60,
  },
});

const StyledCentered = styled.div({
  display: 'flex',
  flexDirection: 'column',
  alignItems: 'center',
  gap: spacings.medium,
  padding: `${spacings.big} ${spacings.medium}`,
  textAlign: 'center',
});

const StyledMessage = styled.p({
  margin: 0,
  maxWidth: '32ch',
  color: colors.whiteOnDarkBlue60,
  fontSize: '13px',
  lineHeight: '19px',
});

/**
 * Live figures of the Warren network: the fleet, then one card per exit, then
 * how the figures are made. Polls only while shown and focused.
 */
export function NetworkView() {
  const { pop } = useHistory();
  const { stats, exitsByHostname, stale, status } = useWarrenNetworkStats();
  const places = useExitPlaces(exitsByHostname);

  const bandOnlyExits = stats?.exits.filter((exit) => exit.online && !exit.live).length ?? 0;

  return (
    <View backgroundColor="darkBlue">
      <BackAction action={pop}>
        <NavigationContainer>
          <AppNavigationHeader
            title={
              // TRANSLATORS: Title of the view with the live figures of the Warren network.
              messages.pgettext('network-stats', 'Warren network')
            }
            titleVisible
          />
          <NavigationScrollbars>
            <View.Content>
              {stats === undefined ? (
                <StyledCentered>
                  {status === 'idle' ? (
                    <Spinner size="big" />
                  ) : (
                    <StyledMessage>
                      {
                        // TRANSLATORS: Shown when no network figures could be read.
                        messages.pgettext(
                          'network-stats',
                          'The network figures are not available right now.',
                        )
                      }
                    </StyledMessage>
                  )}
                </StyledCentered>
              ) : (
                <StyledStack data-testid="network-view">
                  <FleetCard stats={stats} stale={stale} />

                  <StyledSectionTitle>
                    {
                      // TRANSLATORS: Heading of the list of Warren exits.
                      messages.pgettext('network-stats', 'Exits')
                    }
                  </StyledSectionTitle>
                  {bandOnlyExits > 0 && (
                    <StyledNote>
                      <InfoGlyph />
                      {liveThresholdNote(stats.exitLiveThreshold)}
                    </StyledNote>
                  )}
                  {stats.exits.map((exit) => (
                    <ExitCard
                      key={exit.exitId}
                      exit={exit}
                      stats={stats}
                      place={places.get(exit.exitId)}
                      stale={stale}
                    />
                  ))}

                  <Methodology stats={stats} />
                </StyledStack>
              )}
            </View.Content>
          </NavigationScrollbars>
        </NavigationContainer>
      </BackAction>
    </View>
  );
}
