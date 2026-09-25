import React, { useCallback } from 'react';
import styled from 'styled-components';

import { messages } from '../../../../../../../../shared/gettext';
import {
  exitDisplayMode,
  ExitStats,
  NetworkStats,
} from '../../../../../../../../shared/network-stats';
import { RoutePath } from '../../../../../../../../shared/routes';
import { colors, Radius } from '../../../../../../../lib/foundations';
import { TransitionType, useHistory } from '../../../../../../../lib/history';
import { liveThresholdNote, useWarrenNetworkStats } from '../../../../../../../lib/network-stats';
import { useSelector } from '../../../../../../../redux/store';
import {
  ExitLoadSummary,
  InfoGlyph,
  LiveIndicator,
  Sparkline,
  ThroughputPair,
} from '../../../../../../network-stats';

// The exit carrying the tunnel, as the snapshot describes it. Mounted only
// while connected, so the stats are asked for only while the user can see them.
function useConnectedExit():
  | { exit: ExitStats; stats: NetworkStats; stale: boolean; locale: string }
  | undefined {
  const hostname = useSelector((state) => state.connection.hostname);
  const locale = useSelector((state) => state.userInterface.locale);
  const { stats, exitsByHostname, stale } = useWarrenNetworkStats();
  const exit = hostname === undefined ? undefined : exitsByHostname.get(hostname);
  return stats === undefined || exit === undefined ? undefined : { exit, stats, stale, locale };
}

function useOpenNetworkView() {
  const history = useHistory();
  return useCallback(
    (event: React.MouseEvent) => {
      // The card header toggles the details on click; this must not.
      event.stopPropagation();
      history.push(RoutePath.warrenNetwork, { transition: TransitionType.show });
    },
    [history],
  );
}

const openNetworkLabel = () =>
  // TRANSLATORS: Accessibility label of the exit load on the connection card,
  // TRANSLATORS: which opens the live figures of the Warren network.
  messages.pgettext('network-stats', 'Show the Warren network');

const StyledCompactButton = styled.button({
  display: 'flex',
  alignItems: 'center',
  flexShrink: 0,
  height: '20px',
  padding: 0,
  border: 'none',
  background: 'none',
  cursor: 'pointer',
  borderRadius: Radius.radius4,
  '&:focus-visible': {
    outline: `2px solid ${colors.whiteAlpha60}`,
    outlineOffset: '2px',
  },
});

/**
 * The exit's load at the trailing edge of the hostname line. It shares that
 * line instead of adding one: the card is anchored to the bottom, so a new
 * row would lift its top edge over the scenery.
 */
export function ConnectedExitLoad() {
  const connected = useConnectedExit();
  const open = useOpenNetworkView();
  if (connected === undefined) {
    return null;
  }
  const { exit, stats, stale, locale } = connected;
  return (
    <StyledCompactButton
      type="button"
      onClick={open}
      aria-label={openNetworkLabel()}
      data-testid="connected-exit-load">
      <ExitLoadSummary exit={exit} stats={stats} ringSize="tiny" locale={locale} stale={stale} />
    </StyledCompactButton>
  );
}

const StyledLiveBlock = styled.button({
  display: 'flex',
  flexDirection: 'column',
  gap: '6px',
  width: '100%',
  marginTop: '12px',
  padding: '10px 12px',
  border: `1px solid ${colors.whiteAlpha20}`,
  borderRadius: Radius.radius12,
  background: 'none',
  textAlign: 'left',
  cursor: 'pointer',
  '&:hover': {
    borderColor: colors.whiteAlpha40,
  },
  '&:focus-visible': {
    outline: `2px solid ${colors.whiteAlpha60}`,
    outlineOffset: '2px',
  },
});

const StyledBlockRow = styled.span({
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'space-between',
  gap: '8px',
});

const StyledBlockHeading = styled.span({
  color: colors.whiteAlpha60,
  fontFamily: 'var(--font-family-open-sans)',
  fontSize: '10px',
  fontWeight: 600,
  lineHeight: '15px',
});

const StyledNote = styled.span({
  display: 'flex',
  alignItems: 'flex-start',
  gap: '6px',
  color: colors.whiteAlpha60,
  fontFamily: 'var(--font-family-open-sans)',
  fontSize: '11px',
  lineHeight: '16px',
  '& > svg': {
    flexShrink: 0,
    marginTop: '2px',
  },
});

const StyledSparkline = styled.span({
  display: 'block',
  width: '100%',
});

/**
 * The exit's live figures in the expanded connection details: load, people,
 * throughput and the last hour, or the band alone on a quiet exit.
 */
export function ConnectedExitLive() {
  const connected = useConnectedExit();
  const open = useOpenNetworkView();
  if (connected === undefined) {
    return null;
  }
  const { exit, stats, stale, locale } = connected;
  const live = exitDisplayMode(exit) === 'live';
  return (
    <StyledLiveBlock
      type="button"
      onClick={open}
      aria-label={openNetworkLabel()}
      data-testid="connected-exit-live">
      <StyledBlockRow>
        <StyledBlockHeading>
          {
            // TRANSLATORS: Heading of the live figures of the exit carrying the tunnel.
            messages.pgettext('network-stats', 'Your exit, live')
          }
        </StyledBlockHeading>
        <LiveIndicator stats={stats} stale={stale} />
      </StyledBlockRow>
      <StyledBlockRow>
        <ExitLoadSummary exit={exit} stats={stats} ringSize="small" locale={locale} stale={stale} />
        {live && (
          <ThroughputPair
            downloadBps={exit.downloadBps}
            uploadBps={exit.uploadBps}
            locale={locale}
          />
        )}
      </StyledBlockRow>
      {live && exit.history.length > 0 && (
        <StyledSparkline>
          <Sparkline
            values={exit.history.map((point) => point.throughputBps)}
            width={240}
            height={28}
          />
        </StyledSparkline>
      )}
      {exitDisplayMode(exit) === 'band' && (
        <StyledNote>
          <InfoGlyph />
          {liveThresholdNote(stats.exitLiveThreshold)}
        </StyledNote>
      )}
    </StyledLiveBlock>
  );
}
