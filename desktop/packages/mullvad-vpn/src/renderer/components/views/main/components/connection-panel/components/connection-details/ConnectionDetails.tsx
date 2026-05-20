import { useEffect, useState } from 'react';
import styled from 'styled-components';

import { strings } from '../../../../../../../../shared/constants';
import {
  EndpointObfuscationType,
  ITunnelEndpoint,
  parseSocketAddress,
  RelayProtocol,
  TunnelState,
} from '../../../../../../../../shared/daemon-rpc-types';
import { messages } from '../../../../../../../../shared/gettext';
import { colors } from '../../../../../../../lib/foundations';
import { useSelector } from '../../../../../../../redux/store';
import { tinyText } from '../../../../../../common-styles';

interface Endpoint {
  ip: string;
  port: number;
  protocol: RelayProtocol;
}

interface ObfuscationData extends Endpoint {
  obfuscationType: EndpointObfuscationType;
}

const StyledConnectionDetailsHeading = styled.h2(tinyText, {
  margin: '0 0 4px',
  fontSize: '10px',
  lineHeight: '15px',
  color: colors.whiteAlpha60,
});

const StyledConnectionDetailsContainer = styled.div({
  marginTop: '16px',
  marginBottom: '16px',
});

const StyledIpTable = styled.div({
  display: 'grid',
  gridTemplateColumns: 'minmax(48px, min-content) auto',
});

const StyledIpLabelContainer = styled.div({
  display: 'flex',
  flexDirection: 'column',
});

const StyledConnectionDetailsLabel = styled.span(tinyText, {
  display: 'block',
  color: colors.white,
  fontWeight: '400',
  minHeight: '1em',
});

const StyledConnectionDetailsTitle = styled(StyledConnectionDetailsLabel)({
  color: colors.whiteAlpha60,
  whiteSpace: 'nowrap',
});

export function ConnectionDetails() {
  const reduxConnection = useSelector((state) => state.connection);
  const warrenMode = useSelector((state) => state.settings.warrenMode);
  const [connection, setConnection] = useState(reduxConnection);

  const tunnelState = connection.status;

  useEffect(() => {
    if (
      reduxConnection.status.state === 'connected' ||
      reduxConnection.status.state === 'connecting'
    ) {
      setConnection(reduxConnection);
    }
  }, [reduxConnection, tunnelState.state]);

  const entry = getEntryPoint(tunnelState);

  const showDetails = tunnelState.state === 'connected' || tunnelState.state === 'connecting';
  const hasEntry = showDetails && entry !== undefined;

  return (
    <StyledConnectionDetailsContainer>
      <StyledConnectionDetailsHeading>
        {messages.pgettext('connect-view', 'Connection details')}
      </StyledConnectionDetailsHeading>
      <StyledConnectionDetailsLabel data-testid="tunnel-protocol">
        {showDetails &&
          tunnelState.details !== undefined &&
          (warrenMode ? strings.iroh : strings.wireguard)}
      </StyledConnectionDetailsLabel>
      <StyledIpTable>
        <StyledConnectionDetailsTitle>
          {messages.pgettext('connection-info', 'In')}
        </StyledConnectionDetailsTitle>
        <StyledConnectionDetailsLabel data-testid="in-ip">
          {hasEntry ? `${entry.ip}:${entry.port} ${entry.protocol.toUpperCase()}` : ''}
        </StyledConnectionDetailsLabel>
        <StyledConnectionDetailsTitle>
          {messages.pgettext('connection-info', 'Out')}
        </StyledConnectionDetailsTitle>
        <StyledIpLabelContainer>
          {connection.ipv4 && (
            <StyledConnectionDetailsLabel data-testid="out-ip">
              {connection.ipv4}
            </StyledConnectionDetailsLabel>
          )}
          {connection.ipv6 && (
            <StyledConnectionDetailsLabel data-testid="out-ip">
              {connection.ipv6}
            </StyledConnectionDetailsLabel>
          )}
        </StyledIpLabelContainer>
      </StyledIpTable>
      {warrenMode && <WarrenStatusRows />}
    </StyledConnectionDetailsContainer>
  );
}

// Warren status block: reconnect counter + age + M4.0 obfuscation
// indicator. Reads from `state.settings.warrenStatus` which is fed by
// the daemon WarrenStatusUpdates push stream. Hidden when warren_mode
// is OFF (= pure Mullvad upstream path).
function WarrenStatusRows() {
  const warrenStatus = useSelector((state) => state.settings.warrenStatus);
  const reconnectCount = warrenStatus?.reconnectCount ?? 0;
  const lastReconnectAgeMs = warrenStatus?.lastReconnectAgeMs ?? null;
  // Obfuscation default-on per /v1 doctrine; default to true if the
  // status has not been pushed yet so the UI does not flash an
  // alarming "OFF" state during boot.
  const obfuscationActive = warrenStatus?.obfuscationActive ?? true;

  return (
    <StyledIpTable style={{ marginTop: '8px' }} data-anchor-id="warren-status-reconnect">
      <StyledConnectionDetailsTitle>
        {messages.pgettext('warren-status-view', 'Reconnects')}
      </StyledConnectionDetailsTitle>
      <StyledConnectionDetailsLabel data-testid="warren-reconnect-count">
        {reconnectCount}
      </StyledConnectionDetailsLabel>
      <StyledConnectionDetailsTitle>
        {messages.pgettext('warren-status-view', 'Last')}
      </StyledConnectionDetailsTitle>
      <StyledConnectionDetailsLabel data-testid="warren-last-reconnect-age">
        {lastReconnectAgeMs === null
          ? messages.pgettext('warren-status-view', 'never')
          : formatAge(lastReconnectAgeMs)}
      </StyledConnectionDetailsLabel>
      <StyledConnectionDetailsTitle>
        {messages.pgettext('warren-status-view', 'Obfuscation')}
      </StyledConnectionDetailsTitle>
      <StyledConnectionDetailsLabel data-testid="warren-obfuscation-active">
        {obfuscationActive
          ? messages.pgettext('warren-status-view', 'HTTP/3 mimicry (active)')
          : messages.pgettext('warren-status-view', 'inactive')}
      </StyledConnectionDetailsLabel>
    </StyledIpTable>
  );
}

// Compact age formatter: under a minute = seconds, under an hour =
// minutes:seconds, beyond = HhMM. Tuned for the connection-details
// panel where vertical space is tight.
function formatAge(ms: number): string {
  const totalSec = Math.floor(ms / 1000);
  if (totalSec < 60) {
    return `${totalSec}s ago`;
  }
  if (totalSec < 3600) {
    const m = Math.floor(totalSec / 60);
    const s = totalSec % 60;
    return `${m}m ${s}s ago`;
  }
  const h = Math.floor(totalSec / 3600);
  const m = Math.floor((totalSec % 3600) / 60);
  return `${h}h ${m}m ago`;
}

function getEntryPoint(tunnelState: TunnelState): Endpoint | undefined {
  if (
    (tunnelState.state !== 'connected' && tunnelState.state !== 'connecting') ||
    tunnelState.details === undefined
  ) {
    return undefined;
  }

  const endpoint = tunnelState.details.endpoint;
  const inAddress = tunnelEndpointToRelayInAddress(endpoint);
  const entryLocationInAddress = tunnelEndpointToEntryLocationInAddress(endpoint);
  const obfuscationEndpoint = tunnelEndpointToObfuscationEndpoint(endpoint);

  if (obfuscationEndpoint) {
    return obfuscationEndpoint;
  } else if (entryLocationInAddress && inAddress) {
    return entryLocationInAddress;
  } else {
    return inAddress;
  }
}

function tunnelEndpointToRelayInAddress(tunnelEndpoint: ITunnelEndpoint): Endpoint {
  const socketAddr = parseSocketAddress(tunnelEndpoint.address);
  return {
    ip: socketAddr.host,
    port: socketAddr.port,
    protocol: tunnelEndpoint.protocol,
  };
}

function tunnelEndpointToEntryLocationInAddress(
  tunnelEndpoint: ITunnelEndpoint,
): Endpoint | undefined {
  if (!tunnelEndpoint.entryEndpoint) {
    return undefined;
  }

  const socketAddr = parseSocketAddress(tunnelEndpoint.entryEndpoint.address);
  return {
    ip: socketAddr.host,
    port: socketAddr.port,
    protocol: tunnelEndpoint.entryEndpoint.transportProtocol,
  };
}

function tunnelEndpointToObfuscationEndpoint(
  endpoint: ITunnelEndpoint,
): ObfuscationData | undefined {
  if (!endpoint.obfuscationEndpoint) {
    return undefined;
  }

  const socketAddr = parseSocketAddress(endpoint.obfuscationEndpoint.address);

  return {
    ip: socketAddr.host,
    port: socketAddr.port,
    protocol: endpoint.obfuscationEndpoint.protocol,
    obfuscationType: endpoint.obfuscationEndpoint.obfuscationType,
  };
}
