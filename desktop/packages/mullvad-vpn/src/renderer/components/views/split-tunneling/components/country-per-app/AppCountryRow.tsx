import React from 'react';
import { sprintf } from 'sprintf-js';
import styled from 'styled-components';

import { AppRouteLine } from '../../../../../../shared/app-routing';
import { ISplitTunnelingApplication } from '../../../../../../shared/application-types';
import { ExitChoice } from '../../../../../../shared/daemon-rpc-types';
import { messages } from '../../../../../../shared/gettext';
import { CountryChip, RouteStatusLine } from '../../../../../features/app-routing/components';
import { routingLimitationText } from '../../../../../features/app-routing/strings';
import { IconButton } from '../../../../../lib/components';
import { colors, spacings } from '../../../../../lib/foundations';
import { Container } from '../../../../cell';
import { normalText, tinyText } from '../../../../common-styles';
import { ApplicationIcon } from '../application-icon';

const StyledRow = styled(Container)({
  backgroundColor: colors.blue40,
  // Rows with a status line keep one height, so the list never jumps when a
  // route changes state.
  minHeight: '56px',
});

const StyledText = styled.div({
  flex: 1,
  minWidth: 0,
  display: 'flex',
  flexDirection: 'column',
  justifyContent: 'center',
  padding: `8px ${spacings.small} 8px 0`,
});

const StyledActions = styled.div({
  display: 'flex',
  alignItems: 'center',
  gap: spacings.small,
});

const StyledName = styled.span({
  ...normalText,
  color: colors.white,
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  whiteSpace: 'nowrap',
});

// Unlike a route status, this line never changes, so it may wrap.
const StyledLimitation = styled.span({
  ...tinyText,
  fontWeight: 400,
  color: colors.whiteAlpha60,
});

export type AppCountryRowProps = {
  application: ISplitTunnelingApplication;
  exit?: ExitChoice;
  exitLabel?: string;
  line?: AppRouteLine;
  onPick: (application: ISplitTunnelingApplication) => void;
  onClear?: (application: ISplitTunnelingApplication) => void;
};

export function AppCountryRow({
  application,
  exit,
  exitLabel,
  line,
  onPick,
  onClear,
}: AppCountryRowProps) {
  const pick = React.useCallback(() => onPick(application), [application, onPick]);
  const clear = React.useCallback(() => onClear?.(application), [application, onClear]);

  const chipLabel =
    exit !== undefined && exitLabel !== undefined
      ? exitLabel
      : // TRANSLATORS: Button on an app row opening the country picker.
        messages.pgettext('split-tunneling-view', 'Country');

  const chipAccessibleLabel =
    exit !== undefined && exitLabel !== undefined
      ? sprintf(
          // TRANSLATORS: Accessibility label of the country button of an app.
          // TRANSLATORS: Available placeholders:
          // TRANSLATORS: %(application)s - the app's name
          // TRANSLATORS: %(country)s - the country (and city) it leaves from
          messages.pgettext(
            'split-tunneling-view',
            'Change the country of %(application)s, now %(country)s',
          ),
          { application: application.name, country: exitLabel },
        )
      : sprintf(
          // TRANSLATORS: Accessibility label of the country button of an app.
          // TRANSLATORS: Available placeholders:
          // TRANSLATORS: %(application)s - the app's name
          messages.pgettext('split-tunneling-view', 'Choose a country for %(application)s'),
          { application: application.name },
        );

  const limitation =
    application.routingLimitation && routingLimitationText(application.routingLimitation);

  return (
    <StyledRow data-testid="app-country-row">
      <ApplicationIcon icon={application.icon} />
      <StyledText>
        <StyledName title={application.name}>{application.name}</StyledName>
        {limitation ? (
          <StyledLimitation>{limitation}</StyledLimitation>
        ) : (
          line && <RouteStatusLine line={line} />
        )}
      </StyledText>
      <StyledActions>
        <CountryChip
          country={exit?.country}
          label={chipLabel}
          accessibleLabel={chipAccessibleLabel}
          disabled={limitation !== undefined}
          onClick={pick}
        />
        {onClear && (
          <IconButton
            variant="secondary"
            aria-label={sprintf(
              // TRANSLATORS: Accessibility label of the button removing the
              // TRANSLATORS: country of an app. Available placeholders:
              // TRANSLATORS: %(application)s - the app's name
              messages.pgettext('split-tunneling-view', 'Remove the country of %(application)s'),
              { application: application.name },
            )}
            onClick={clear}>
            <IconButton.Icon icon="remove-circle" />
          </IconButton>
        )}
      </StyledActions>
    </StyledRow>
  );
}
