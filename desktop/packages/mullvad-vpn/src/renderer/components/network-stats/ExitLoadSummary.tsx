import { sprintf } from 'sprintf-js';
import styled from 'styled-components';

import { messages } from '../../../shared/gettext';
import {
  exitDisplayMode,
  ExitStats,
  exitUsersLabel,
  formatPercent,
  NetworkStats,
} from '../../../shared/network-stats';
import { colors } from '../../lib/foundations';
import { loadLevelLabel } from '../../lib/network-stats';
import { LoadRing, LoadRingSize } from './LoadRing';
import { UsersPill } from './UsersPill';

const StyledSummary = styled.span<{ $muted: boolean }>(({ $muted }) => ({
  display: 'inline-flex',
  alignItems: 'center',
  gap: '6px',
  flexShrink: 0,
  opacity: $muted ? 0.5 : 1,
  transition: 'opacity 300ms ease-out',
}));

const StyledValue = styled.span({
  color: colors.whiteAlpha80,
  fontFamily: 'var(--font-family-open-sans)',
  fontSize: '12px',
  fontWeight: 600,
  lineHeight: '18px',
  whiteSpace: 'nowrap',
  fontVariantNumeric: 'tabular-nums',
});

export type ExitLoadSummaryProps = {
  exit: ExitStats;
  stats: Pick<NetworkStats, 'exitUsersRounding' | 'exitLiveThreshold'>;
  ringSize: LoadRingSize;
  locale: string;
  // The last good snapshot is old: keep it on screen, greyed.
  stale?: boolean;
};

/**
 * The load of one exit at a glance: the ring, the percentage or the band name
 * (colour is never the only signal), and how many people share it.
 */
export function ExitLoadSummary({ exit, stats, ringSize, locale, stale }: ExitLoadSummaryProps) {
  const mode = exitDisplayMode(exit);

  if (mode === 'offline') {
    return (
      <StyledSummary $muted data-testid="exit-load-summary">
        <LoadRing size={ringSize} level="unknown" muted aria-hidden />
        <StyledValue>
          {
            // TRANSLATORS: A Warren exit that is not serving right now.
            messages.pgettext('network-stats', 'Offline')
          }
        </StyledValue>
      </StyledSummary>
    );
  }

  const percent = mode === 'live' ? exit.loadPercent : undefined;
  const value =
    percent === undefined ? loadLevelLabel(exit.loadLevel) : formatPercent(percent, locale);
  const users = exitUsersLabel(exit, stats);

  return (
    <StyledSummary
      $muted={stale === true}
      data-testid="exit-load-summary"
      aria-label={sprintf(
        // TRANSLATORS: Accessibility label of the load of one Warren exit.
        // TRANSLATORS: Available placeholders:
        // TRANSLATORS: %(load)s - the load, e.g. "37%" or "Low load"
        // TRANSLATORS: %(people)s - how many people use the exit, e.g. "40+" or "< 20"
        messages.pgettext('network-stats', 'Load %(load)s, %(people)s people'),
        { load: value, people: users },
      )}>
      <LoadRing size={ringSize} level={exit.loadLevel} percent={percent} aria-hidden />
      <StyledValue aria-hidden>{value}</StyledValue>
      <UsersPill label={users} aria-hidden />
    </StyledSummary>
  );
}
