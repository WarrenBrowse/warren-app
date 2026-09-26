import { sprintf } from 'sprintf-js';
import styled from 'styled-components';

import { messages } from '../../../../../shared/gettext';
import { NetworkStats } from '../../../../../shared/network-stats';
import { colors, spacings } from '../../../../lib/foundations';

const StyledSection = styled.section({
  display: 'flex',
  flexDirection: 'column',
  gap: spacings.small,
  marginTop: spacings.medium,
});

const StyledHeading = styled.h2({
  margin: 0,
  color: colors.white,
  fontFamily: 'var(--font-family-open-sans)',
  fontSize: '14px',
  fontWeight: 600,
  lineHeight: '20px',
});

const StyledList = styled.ul({
  display: 'flex',
  flexDirection: 'column',
  gap: '6px',
  margin: 0,
  paddingLeft: '18px',
  listStyleType: 'disc',
  color: colors.whiteAlpha80,
  '& > li::marker': {
    color: colors.whiteAlpha40,
  },
  fontFamily: 'var(--font-family-open-sans)',
  fontSize: '12px',
  lineHeight: '18px',
});

export function Methodology({ stats }: { stats: NetworkStats }) {
  return (
    <StyledSection data-testid="network-methodology">
      <StyledHeading>
        {
          // TRANSLATORS: Heading of the note on how the network figures are made.
          messages.pgettext('network-stats', 'How these numbers are computed')
        }
      </StyledHeading>
      <StyledList>
        <li>
          {sprintf(
            // TRANSLATORS: Available placeholders:
            // TRANSLATORS: %(seconds)d - length of one measuring window, in seconds
            messages.pgettext(
              'network-stats',
              'Live figures cover one closed window of %(seconds)d seconds and change only when the next one closes. The 24-hour figures are counted by clock hour.',
            ),
            { seconds: stats.windowSecs },
          )}
        </li>
        <li>
          {sprintf(
            // TRANSLATORS: Available placeholders:
            // TRANSLATORS: %(threshold)d - fewest people an exit must carry for its live figures to show
            messages.pgettext(
              'network-stats',
              'An exit shows live figures only while at least %(threshold)d people use it, so no one’s traffic can be singled out. Below that, it shows only its load band over the last hour, and while other exits are live it enters the network totals by the hour.',
            ),
            { threshold: stats.exitLiveThreshold },
          )}
        </li>
        <li>
          {sprintf(
            // TRANSLATORS: Available placeholders:
            // TRANSLATORS: %(step)d - the step per-exit people counts are rounded down to
            messages.pgettext(
              'network-stats',
              'People per exit are rounded down to steps of %(step)d. The total for the whole network is exact.',
            ),
            { step: stats.exitUsersRounding },
          )}
        </li>
        <li>
          {messages.pgettext(
            'network-stats',
            'Load is the busier direction of the link against its capacity, or the processor use, whichever is closer to saturation.',
          )}
        </li>
      </StyledList>
    </StyledSection>
  );
}
