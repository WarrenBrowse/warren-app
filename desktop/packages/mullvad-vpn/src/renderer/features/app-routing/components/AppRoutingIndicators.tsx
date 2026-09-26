import React from 'react';
import { sprintf } from 'sprintf-js';
import styled from 'styled-components';

import { appRoutingSummary } from '../../../../shared/app-routing';
import { messages } from '../../../../shared/gettext';
import { tinyText } from '../../../components/common-styles';
import { FeatureIndicator, Icon } from '../../../lib/components';
import { colors } from '../../../lib/foundations';
import { useSelector } from '../../../redux/store';
import { useAppRouting, useOpenAppRouting } from '../hooks';

const StyledWrapper = styled.div({
  display: 'flex',
});

// Badge above the connection card while some apps leave from their own
// country. Red when one of those routes cannot run for a reason of its own.
export function AppCountriesIndicator() {
  const { routing, statuses, platform } = useAppRouting();
  const tunnelState = useSelector((state) => state.connection.status.state);
  const openAppRouting = useOpenAppRouting();
  const open = React.useCallback(() => openAppRouting('countries'), [openAppRouting]);

  const { appsWithOwnCountry, anyRouteUnavailable } = appRoutingSummary(
    routing,
    statuses,
    platform,
  );
  // The routes live inside the main connection: without it there is nothing
  // to report yet.
  if (appsWithOwnCountry === 0 || (tunnelState !== 'connected' && tunnelState !== 'connecting')) {
    return null;
  }

  const label = sprintf(
    // TRANSLATORS: Chip on the main screen while some apps leave the
    // TRANSLATORS: Internet from a country of their own. Available placeholders:
    // TRANSLATORS: %(count)d - the number of such apps
    messages.npgettext(
      'connect-view',
      '%(count)d app in another country',
      '%(count)d apps in other countries',
      appsWithOwnCountry,
    ),
    { count: appsWithOwnCountry },
  );

  return (
    <StyledWrapper>
      <FeatureIndicator
        variant={anyRouteUnavailable ? 'error' : 'primary'}
        onClick={open}
        data-testid="app-countries-indicator">
        <FeatureIndicator.Text>{label}</FeatureIndicator.Text>
      </FeatureIndicator>
    </StyledWrapper>
  );
}

const StyledIncludeOnlyLabel = styled.button({
  ...tinyText,
  display: 'inline-flex',
  alignItems: 'center',
  gap: '6px',
  alignSelf: 'flex-start',
  marginTop: '8px',
  paddingBlock: '2px',
  paddingInlineStart: '4px',
  paddingInlineEnd: '8px',
  borderRadius: '6px',
  border: `1px solid color-mix(in srgb, ${colors.yellow} 45%, transparent)`,
  backgroundColor: `color-mix(in srgb, ${colors.yellow} 12%, transparent)`,
  color: colors.white,
  cursor: 'default',
  '&&:hover': {
    borderColor: colors.yellow,
  },
  '&&:focus-visible': {
    outline: `2px solid ${colors.white}`,
    outlineOffset: '1px',
  },
});

// Under the connection state while only chosen apps use the VPN, so the rest
// of the device being unprotected is never a surprise.
export function IncludeOnlyLabel() {
  const { routing, statuses, platform } = useAppRouting();
  const openAppRouting = useOpenAppRouting();

  const open = React.useCallback(
    (event: React.MouseEvent) => {
      // The label sits inside the card header, whose click expands the card.
      event.stopPropagation();
      openAppRouting('include-only');
    },
    [openAppRouting],
  );

  const { includeOnly, vpnOnlyForCount } = appRoutingSummary(routing, statuses, platform);
  if (!includeOnly) {
    return null;
  }

  const label =
    vpnOnlyForCount === undefined
      ? // TRANSLATORS: Label under the connection state while only the apps
        // TRANSLATORS: the user opens from "VPN only for" use the VPN.
        messages.pgettext('connect-view', 'VPN only for chosen apps')
      : sprintf(
          // TRANSLATORS: Label under the connection state while only the chosen
          // TRANSLATORS: apps use the VPN. Available placeholders:
          // TRANSLATORS: %(count)d - the number of apps that use the VPN
          messages.npgettext(
            'connect-view',
            'VPN only for %(count)d app',
            'VPN only for %(count)d apps',
            vpnOnlyForCount,
          ),
          { count: vpnOnlyForCount },
        );

  return (
    <StyledIncludeOnlyLabel type="button" onClick={open} data-testid="include-only-label">
      <Icon icon="alert-circle" size="tiny" color="yellow" aria-hidden />
      {label}
    </StyledIncludeOnlyLabel>
  );
}
