import { sprintf } from 'sprintf-js';
import styled from 'styled-components';

import { messages } from '../../../../../shared/gettext';
import {
  exitDisplayMode,
  ExitStats,
  exitUsersLabel,
  formatBitsPerSecond,
  formatPercent,
  NetworkStats,
  uptimeDays,
} from '../../../../../shared/network-stats';
import { colors, spacings } from '../../../../lib/foundations';
import { loadLevelLabel, useTweenedNumber } from '../../../../lib/network-stats';
import { useSelector } from '../../../../redux/store';
import { LoadRing, Sparkline, ThroughputPair, UsersPill } from '../../../network-stats';
import type { ExitPlace } from '../hooks';
import {
  StyledCard,
  StyledCardHeader,
  StyledLabel,
  StyledRingBand,
  StyledRingCaption,
  StyledRingValue,
} from './styles';

const StyledTitle = styled.h3({
  margin: 0,
  minWidth: 0,
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  whiteSpace: 'nowrap',
  color: colors.white,
  fontFamily: 'var(--font-family-open-sans)',
  fontSize: '14px',
  fontWeight: 600,
  lineHeight: '20px',
});

const StyledBody = styled.div({
  display: 'flex',
  alignItems: 'center',
  gap: spacings.medium,
});

const StyledFacts = styled.div({
  display: 'flex',
  flexDirection: 'column',
  alignItems: 'flex-start',
  gap: '6px',
  minWidth: 0,
});

const StyledFact = styled(StyledLabel)({
  color: colors.whiteAlpha80,
});

function uptimeLabel(uptimeSecs: number): string {
  const days = uptimeDays(uptimeSecs);
  if (days === 0) {
    // TRANSLATORS: An exit that has run for less than one day.
    return messages.pgettext('network-stats', 'Up less than a day');
  }
  return sprintf(
    // TRANSLATORS: How long an exit has run, in whole days.
    // TRANSLATORS: Available placeholders:
    // TRANSLATORS: %(days)d - whole days
    messages.npgettext('network-stats', 'Up %(days)d day', 'Up %(days)d days', days),
    { days },
  );
}

function driverLabel(exit: ExitStats): string | undefined {
  switch (exit.loadDriver) {
    case 'bandwidth':
      // TRANSLATORS: What sets the load of an exit: its network link.
      return messages.pgettext('network-stats', 'Limited by bandwidth');
    case 'cpu':
      // TRANSLATORS: What sets the load of an exit: its processor.
      return messages.pgettext('network-stats', 'Limited by CPU');
    default:
      return undefined;
  }
}

export type ExitCardProps = {
  exit: ExitStats;
  stats: NetworkStats;
  // From the signed relay list; absent for an exit it does not carry.
  place?: ExitPlace;
  stale: boolean;
};

export function ExitCard({ exit, stats, place, stale }: ExitCardProps) {
  const locale = useSelector((state) => state.userInterface.locale);
  const mode = exitDisplayMode(exit);
  const percent = mode === 'live' ? exit.loadPercent : undefined;
  const tweened = useTweenedNumber(percent ?? 0);
  const title = place ? `${place.city}, ${place.country}` : (exit.name ?? exit.city);
  const driver = driverLabel(exit);

  return (
    <StyledCard $muted={stale || mode === 'offline'} data-testid="network-exit-card">
      <StyledCardHeader>
        <StyledTitle>{title}</StyledTitle>
        <StyledLabel>
          {mode === 'offline'
            ? // TRANSLATORS: A Warren exit that is not serving right now.
              messages.pgettext('network-stats', 'Offline')
            : exit.uptimeSecs !== undefined && uptimeLabel(exit.uptimeSecs)}
        </StyledLabel>
      </StyledCardHeader>

      <StyledBody>
        <LoadRing
          size="large"
          level={exit.loadLevel}
          percent={percent}
          muted={mode === 'offline'}
          aria-hidden={mode === 'offline'}
          aria-label={
            mode === 'offline'
              ? undefined
              : percent === undefined
                ? loadLevelLabel(exit.loadLevel)
                : formatPercent(percent, locale)
          }>
          {mode === 'offline' ? undefined : percent === undefined ? (
            <StyledRingBand aria-hidden>{loadLevelLabel(exit.loadLevel)}</StyledRingBand>
          ) : (
            <>
              <StyledRingValue aria-hidden>
                {formatPercent(Math.round(tweened), locale)}
              </StyledRingValue>
              <StyledRingCaption aria-hidden>
                {
                  // TRANSLATORS: Caption under the load percentage of one exit.
                  messages.pgettext('network-stats', 'load')
                }
              </StyledRingCaption>
            </>
          )}
        </LoadRing>

        {mode !== 'offline' && (
          <StyledFacts>
            <UsersPill label={exitUsersLabel(exit, stats)} />
            {mode === 'live' ? (
              <>
                <ThroughputPair
                  downloadBps={exit.downloadBps}
                  uploadBps={exit.uploadBps}
                  locale={locale}
                />
                {driver && <StyledFact>{driver}</StyledFact>}
                <StyledLabel>
                  {[
                    exit.capacityBps !== undefined &&
                      sprintf(
                        // TRANSLATORS: Link capacity of an exit, e.g. "1.0 Gbit/s link".
                        // TRANSLATORS: Available placeholders:
                        // TRANSLATORS: %(capacity)s - the capacity with its unit
                        messages.pgettext('network-stats', '%(capacity)s link'),
                        { capacity: formatBitsPerSecond(exit.capacityBps, locale) },
                      ),
                    exit.cpuPercent !== undefined &&
                      sprintf(
                        // TRANSLATORS: Processor use of an exit, e.g. "CPU 21%".
                        // TRANSLATORS: Available placeholders:
                        // TRANSLATORS: %(percent)s - the percentage with its sign
                        messages.pgettext('network-stats', 'CPU %(percent)s'),
                        { percent: formatPercent(exit.cpuPercent, locale) },
                      ),
                  ]
                    .filter(Boolean)
                    .join(' · ')}
                </StyledLabel>
              </>
            ) : (
              <StyledLabel>
                {
                  // TRANSLATORS: Under the load band of a quiet exit: the band
                  // TRANSLATORS: describes the last closed 15 minutes.
                  messages.pgettext('network-stats', 'Load over the last 15 minutes')
                }
              </StyledLabel>
            )}
          </StyledFacts>
        )}
      </StyledBody>

      {mode === 'live' && exit.history.length > 0 && (
        <Sparkline
          values={exit.history.map((point) => point.throughputBps)}
          width={300}
          height={32}
          color="whiteOnDarkBlue60"
        />
      )}
    </StyledCard>
  );
}
