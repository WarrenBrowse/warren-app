import { useEffect, useState } from 'react';
import styled from 'styled-components';

import { strings } from '../../../../../../../../shared/constants';
import {
  EndpointObfuscationType,
  formatExitLocation,
  isMultihopTunnelState,
  isUnspecifiedSocketAddress,
  ITunnelEndpoint,
  parseSocketAddress,
  RelayProtocol,
  TunnelState,
} from '../../../../../../../../shared/daemon-rpc-types';
import { messages, relayLocations } from '../../../../../../../../shared/gettext';
import { FontFamilies, homeTokens, surfaces } from '../../../../../../../lib/foundations';
import { useSelector } from '../../../../../../../redux/store';

interface Endpoint {
  ip: string;
  port: number;
  protocol: RelayProtocol;
}

interface ObfuscationData extends Endpoint {
  obfuscationType: EndpointObfuscationType;
}

const { details } = homeTokens;

// The mockup's details card: a two-column grid of label and value, the
// addresses in the mono face, the reconnection counters under a hairline.
const StyledCard = styled.div`
  margin-top: ${details.marginTop}px;
  display: grid;
  grid-template-columns: auto 1fr;
  column-gap: ${details.columnGap}px;
  row-gap: ${details.rowGap}px;
  padding: ${details.paddingVertical}px ${details.paddingHorizontal}px;
  border-radius: ${details.radius}px;
  background-color: ${surfaces.homeDetails};
  box-shadow: inset 0 0 0 1px ${surfaces.homeDetailsLine};
  font-family: ${FontFamilies.sans};
  font-size: ${details.textSize}px;
  line-height: 1.25;
  color: ${surfaces.homeTextSecondary};
`;

const StyledLabel = styled.span`
  white-space: nowrap;
`;

const StyledValue = styled.span<{ $mono?: boolean; $strong?: boolean }>`
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
  color: ${surfaces.homeText};
  font-family: ${(props) => (props.$mono ? FontFamilies.mono : FontFamilies.sans)};
  font-size: ${(props) => (props.$mono ? details.monoSize : details.textSize)}px;
  font-weight: ${(props) => (props.$strong ? details.valueWeight : 'normal')};
  overflow-wrap: anywhere;
`;

const StyledDivider = styled.span`
  grid-column: 1 / -1;
  height: 1px;
  margin: ${details.dividerMargin}px 0;
  background-color: ${surfaces.homeDetailsLine};
`;

const StyledObfuscation = styled.span`
  display: flex;
  align-items: center;
  gap: 6px;
  color: ${surfaces.homeText};
`;

const StyledActiveChip = styled.span`
  padding: ${details.chipPaddingVertical}px ${details.chipPaddingHorizontal}px;
  border-radius: 99px;
  background-color: ${surfaces.homeActiveWell};
  color: ${surfaces.homeActive};
  font-size: ${details.chipSize}px;
  font-weight: ${details.chipWeight};
`;

export function ConnectionDetails() {
  const reduxConnection = useSelector((state) => state.connection);
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
  const exit = getExitPoint(tunnelState);

  const showDetails = tunnelState.state === 'connected' || tunnelState.state === 'connecting';
  const hasEntry = showDetails && entry !== undefined;
  const hasExit = showDetails && exit !== undefined;
  // Multi-hop = the relay node and the exit node are DIFFERENT nodes
  // (host/IP differs). A 1-hop circuit reuses the same node with
  // different ports, so the port must be ignored - see
  // `isMultihopTunnelState`.
  const multihop = isMultihopTunnelState(tunnelState);
  // When the egress IP is unavailable (redacted multi-hop exit: the client
  // never learns the exit IP, and Warren does not run an am.i.mullvad-style
  // conncheck), fall back to the exit geolocation the daemon does provide so
  // the "Out" row is not left blank.
  const exitLocationFallback =
    showDetails && !hasExit && !connection.ipv4 && !connection.ipv6
      ? formatExitLocation(
          connection.country && relayLocations.gettext(connection.country),
          connection.city && relayLocations.gettext(connection.city),
        )
      : undefined;

  return (
    <StyledCard data-testid="connection-details">
      <StyledLabel>
        {
          // TRANSLATORS: Label of the tunnel protocol in the connection details.
          messages.pgettext('connection-info', 'Protocol')
        }
      </StyledLabel>
      <StyledValue $strong data-testid="tunnel-protocol">
        {showDetails && tunnelState.details !== undefined && strings.quic}
        {multihop && (
          <span data-testid="multihop-indicator">
            {messages.pgettext('connection-info', 'Multihop (2 hops)')}
          </span>
        )}
      </StyledValue>
      <StyledLabel>{messages.pgettext('connection-info', 'In')}</StyledLabel>
      <StyledValue $mono data-testid="in-ip">
        {hasEntry ? `${entry.ip}:${entry.port} ${entry.protocol.toUpperCase()}` : ''}
      </StyledValue>
      <StyledLabel>{messages.pgettext('connection-info', 'Out')}</StyledLabel>
      <StyledValue $mono>
        {hasExit && (
          <span data-testid="out-endpoint">{`${exit.ip}:${exit.port} ${exit.protocol.toUpperCase()}`}</span>
        )}
        {connection.ipv4 && <span data-testid="out-ip">{connection.ipv4}</span>}
        {connection.ipv6 && <span data-testid="out-ip">{connection.ipv6}</span>}
        {exitLocationFallback && <span data-testid="out-location">{exitLocationFallback}</span>}
      </StyledValue>
      <StyledDivider />
      <WarrenStatusRows />
    </StyledCard>
  );
}

// Warren status block: reconnect counter + age + obfuscation
// indicator. Reads from `state.settings.warrenStatus` which is fed by
// the daemon WarrenStatusUpdates push stream.
function WarrenStatusRows() {
  const warrenStatus = useSelector((state) => state.settings.warrenStatus);
  const reconnectCount = warrenStatus?.reconnectCount ?? 0;
  const lastReconnectAgeMs = warrenStatus?.lastReconnectAgeMs ?? null;
  // Obfuscation default-on per /v1 doctrine; default to true if the
  // status has not been pushed yet so the UI does not flash an
  // alarming "OFF" state during boot.
  const obfuscationActive = warrenStatus?.obfuscationActive ?? true;

  return (
    <>
      <StyledLabel data-anchor-id="warren-status-reconnect">
        {messages.pgettext('warren-status-view', 'Reconnects')}
      </StyledLabel>
      <StyledValue data-testid="warren-reconnect-count">{reconnectCount}</StyledValue>
      {reconnectCount > 0 && (
        <>
          <StyledLabel>{messages.pgettext('warren-status-view', 'Last')}</StyledLabel>
          <StyledValue data-testid="warren-last-reconnect-age">
            {lastReconnectAgeMs === null
              ? messages.pgettext('warren-status-view', 'never')
              : formatAge(lastReconnectAgeMs)}
          </StyledValue>
        </>
      )}
      <StyledLabel>{messages.pgettext('warren-status-view', 'Obfuscation')}</StyledLabel>
      {obfuscationActive ? (
        <StyledObfuscation data-testid="warren-obfuscation-active">
          {
            // TRANSLATORS: The obfuscation the tunnel uses, in the connection
            // TRANSLATORS: details, followed by a small "active" chip.
            messages.pgettext('warren-status-view', 'HTTP/3 mimicry')
          }
          <StyledActiveChip>
            {
              // TRANSLATORS: Small chip after the obfuscation name while it runs.
              messages.pgettext('warren-status-view', 'active')
            }
          </StyledActiveChip>
        </StyledObfuscation>
      ) : (
        <StyledValue data-testid="warren-obfuscation-active">
          {messages.pgettext('warren-status-view', 'inactive')}
        </StyledValue>
      )}
    </>
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

// The "Out" (Sortante) endpoint is the exit the user traffic leaves
// through. The daemon publishes it as the main `endpoint.address`
// (for multi-hop with a disclosed exit it is the exit descriptor's endpoint;
// for single-hop the only hop). The relay/entry endpoint, when present, lives
// in `entryEndpoint` instead. Mirrors the CLI's `<exit> via <entry>` line.
//
// In the standard multi-hop flow the client-facing directory redacts the exit
// egress IP, so the daemon publishes an unspecified `0.0.0.0:0` placeholder as
// the exit endpoint (the client never learns the exit IP). Rendering it
// verbatim showed a bogus "Sortante 0.0.0.0:0 UDP" line; instead we drop the
// endpoint address and let the geoip out IP (connection.ipv4/ipv6) and the
// country/city label identify the exit.
function getExitPoint(tunnelState: TunnelState): Endpoint | undefined {
  if (
    (tunnelState.state !== 'connected' && tunnelState.state !== 'connecting') ||
    tunnelState.details === undefined
  ) {
    return undefined;
  }

  const endpoint = tunnelState.details.endpoint;
  if (isUnspecifiedSocketAddress(endpoint.address)) {
    return undefined;
  }

  return tunnelEndpointToRelayInAddress(endpoint);
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
