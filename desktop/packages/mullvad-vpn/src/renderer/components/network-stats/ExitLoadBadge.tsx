import { sprintf } from 'sprintf-js';
import styled from 'styled-components';

import { messages } from '../../../shared/gettext';
import {
  exitDisplayMode,
  ExitStats,
  exitUsersLabel,
  formatBitsPerSecond,
  formatPercent,
  NetworkStats,
} from '../../../shared/network-stats';
import { colors } from '../../lib/foundations';
import { bandArcPercent, liveThresholdNote, loadLevelLabel } from '../../lib/network-stats';
import { ArrowGlyph, PersonGlyph } from './glyphs';
import { LoadRing } from './LoadRing';

const StyledBadge = styled.span<{ $muted: boolean }>(({ $muted }) => ({
  display: 'inline-flex',
  alignItems: 'center',
  gap: '8px',
  flexShrink: 0,
  color: colors.whiteAlpha60,
  fontFamily: 'var(--font-family-open-sans)',
  fontSize: '12px',
  fontWeight: 600,
  lineHeight: '16px',
  whiteSpace: 'nowrap',
  fontVariantNumeric: 'tabular-nums',
  opacity: $muted ? 0.5 : 1,
  transition: 'opacity 300ms ease-out',
}));

const StyledPart = styled.span({
  display: 'inline-flex',
  alignItems: 'center',
  gap: '4px',
});

export type ExitLoadBadgeProps = {
  exit: ExitStats;
  stats: Pick<NetworkStats, 'exitUsersRounding' | 'exitLiveThreshold'>;
  locale: string;
  // Add the download rate while the exit is live.
  throughput?: boolean;
  // The last good snapshot is old: keep it on screen, greyed.
  stale?: boolean;
};

/**
 * The load of one exit in one line of small text: a ring, the percentage while
 * the exit is live, how many people share it, and optionally its download rate.
 * A quiet exit shows no percentage: its ring is filled by band, and the band is
 * named to screen readers and in the tooltip.
 */
export function ExitLoadBadge({ exit, stats, locale, throughput, stale }: ExitLoadBadgeProps) {
  const mode = exitDisplayMode(exit);

  if (mode === 'offline') {
    return (
      <StyledBadge $muted data-testid="exit-load-badge">
        <LoadRing level="unknown" percent={0} muted />
        {
          // TRANSLATORS: A Warren exit that is not serving right now.
          messages.pgettext('network-stats', 'Offline')
        }
      </StyledBadge>
    );
  }

  const live = mode === 'live' && exit.loadPercent !== undefined;
  const percent = live ? formatPercent(exit.loadPercent!, locale) : undefined;
  const users = exitUsersLabel(exit, stats);
  const label = percent
    ? sprintf(
        // TRANSLATORS: Accessibility label of the load of a busy Warren exit.
        // TRANSLATORS: Available placeholders:
        // TRANSLATORS: %(load)s - the load, e.g. "37%"
        // TRANSLATORS: %(people)s - how many people use the exit, e.g. "40+"
        messages.pgettext('network-stats', 'Load %(load)s, %(people)s people'),
        { load: percent, people: users },
      )
    : sprintf(
        // TRANSLATORS: Accessibility label of the load of a quiet Warren exit.
        // TRANSLATORS: Available placeholders:
        // TRANSLATORS: %(band)s - the load band, e.g. "Low load"
        // TRANSLATORS: %(people)s - how many people use the exit, e.g. "< 20"
        messages.pgettext('network-stats', '%(band)s, %(people)s people'),
        { band: loadLevelLabel(exit.loadLevel), people: users },
      );

  return (
    <StyledBadge
      $muted={stale === true}
      role="img"
      aria-label={label}
      title={percent ? label : `${label}. ${liveThresholdNote(stats.exitLiveThreshold)}`}
      data-testid="exit-load-badge">
      <StyledPart>
        <LoadRing
          level={exit.loadLevel}
          percent={live ? exit.loadPercent! : bandArcPercent(exit.loadLevel)}
        />
        {percent}
      </StyledPart>
      <StyledPart>
        <PersonGlyph />
        {users}
      </StyledPart>
      {throughput && live && (
        <StyledPart>
          <ArrowGlyph direction="down" />
          {formatBitsPerSecond(exit.downloadBps, locale)}
        </StyledPart>
      )}
    </StyledBadge>
  );
}
