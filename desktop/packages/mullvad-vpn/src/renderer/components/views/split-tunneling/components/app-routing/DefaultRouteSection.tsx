import React from 'react';
import styled from 'styled-components';

import { type DefaultRoute, defaultRoute } from '../../../../../../shared/app-routing';
import { messages } from '../../../../../../shared/gettext';
import { useAppContext } from '../../../../../context';
import { useAppRouting } from '../../../../../features/app-routing/hooks';
import { Link } from '../../../../../lib/components/link';
import { colors } from '../../../../../lib/foundations';
import { sourceSansPro } from '../../../../common-styles';
import { useRoutingActions } from '../../hooks/use-routing-actions';
import { useRestartDaemon } from '../split-tunneling-settings/components/macos-split-tunneling-availability/hooks';
import { useSplitModeAvailability } from '../split-tunneling-settings/hooks';
import { InfoGlyph } from './glyphs';
import { outsideUnavailableNote, outsideVpnLabel, throughVpnLabel } from './strings';
import { StyledHelp, StyledSection, StyledSectionTitle } from './styles';

const StyledSegments = styled.div({
  display: 'grid',
  gridTemplateColumns: 'repeat(2, minmax(0, 1fr))',
  backgroundColor: colors.blue40,
  borderRadius: '10px',
  overflow: 'hidden',
});

const StyledSegment = styled.button<{ $selected: boolean }>((props) => ({
  ...sourceSansPro,
  minHeight: '48px',
  padding: '4px 8px',
  border: 'none',
  fontSize: '18px',
  lineHeight: '22px',
  fontWeight: props.$selected ? 600 : 400,
  textAlign: 'center',
  backgroundColor: props.$selected ? colors.green80 : colors.transparent,
  color: colors.white,
  cursor: 'default',
  '&&:not(:disabled):hover': {
    backgroundColor: props.$selected ? colors.green80 : colors.blue60,
  },
  '&&:disabled': {
    color: colors.whiteOnDarkBlue20,
  },
  '&&:focus-visible': {
    outline: `2px solid ${colors.white}`,
    outlineOffset: '-2px',
    borderRadius: '10px',
  },
}));

const StyledNote = styled(StyledHelp)({
  display: 'flex',
  gap: '8px',
  alignItems: 'flex-start',
  '& > svg': {
    marginTop: '1px',
  },
});

const StyledNoteBody = styled.span({
  display: 'flex',
  flexDirection: 'column',
  alignItems: 'flex-start',
  gap: '4px',
});

// Full Disk Access is the one missing piece the user can add: one line with
// the way to the pane, and the restart for a grant the service has not seen.
function FullDiskAccessNote() {
  const { showFullDiskAccessSettings } = useAppContext();
  const restartDaemon = useRestartDaemon();
  const [opened, setOpened] = React.useState(false);
  const open = React.useCallback(() => {
    setOpened(true);
    void showFullDiskAccessSettings();
  }, [showFullDiskAccessSettings]);

  return (
    <StyledNote role="note">
      <InfoGlyph />
      <StyledNoteBody>
        <span>
          {messages.pgettext(
            'split-tunneling-view',
            'To use this, enable “Full disk access” for “Warren VPN” in the macOS system settings.',
          )}
        </span>
        <Link as="button" variant="labelTinySemiBold" onClick={open}>
          <Link.Text>{messages.pgettext('split-tunneling-view', 'Open System Settings')}</Link.Text>
        </Link>
        {opened && (
          <>
            <span>
              {messages.pgettext(
                'split-tunneling-view',
                'Enabled "Full disk access" and still having issues?',
              )}
            </span>
            <Link as="button" variant="labelTinySemiBold" onClick={restartDaemon}>
              <Link.Text>
                {messages.pgettext('split-tunneling-view', 'Restart Warren Service')}
              </Link.Text>
            </Link>
          </>
        )}
      </StyledNoteBody>
    </StyledNote>
  );
}

// "Other apps go": what an app without a rule does. Outside the VPN is the
// former "VPN only for" mode, so it needs what that mode needed; going back
// through the VPN is never refused.
export function DefaultRouteSection() {
  const { routing, platform } = useAppRouting();
  const { setDefaultRoute } = useRoutingActions();
  const availability = useSplitModeAvailability();
  const current = defaultRoute(routing);
  const outsideDisabled = current !== 'direct' && availability !== 'available';
  const note = current === 'direct' ? undefined : outsideUnavailableNote(availability, platform);
  const titleId = React.useId();

  const choose = React.useCallback(
    (route: DefaultRoute) => {
      if (route !== current) {
        void setDefaultRoute(route);
      }
    },
    [current, setDefaultRoute],
  );
  const chooseVpn = React.useCallback(() => choose('vpn'), [choose]);
  const chooseOutside = React.useCallback(() => choose('direct'), [choose]);

  return (
    <StyledSection>
      <StyledSectionTitle id={titleId}>
        {
          // TRANSLATORS: Title above the switch choosing what the apps without
          // TRANSLATORS: a rule do: "Through the VPN" or "Outside the VPN".
          messages.pgettext('split-tunneling-view', 'Other apps go')
        }
      </StyledSectionTitle>
      <StyledSegments role="group" aria-labelledby={titleId}>
        <StyledSegment
          type="button"
          aria-pressed={current === 'vpn'}
          $selected={current === 'vpn'}
          onClick={chooseVpn}>
          {throughVpnLabel()}
        </StyledSegment>
        <StyledSegment
          type="button"
          aria-pressed={current === 'direct'}
          $selected={current === 'direct'}
          disabled={outsideDisabled}
          onClick={chooseOutside}>
          {outsideVpnLabel()}
        </StyledSegment>
      </StyledSegments>
      <StyledHelp>
        {current === 'vpn'
          ? messages.pgettext(
              'split-tunneling-view',
              'All apps are protected by the VPN, except for the rules below.',
            )
          : messages.pgettext(
              'split-tunneling-view',
              'Direct connection by default. Only the apps below use the VPN.',
            )}
      </StyledHelp>
      {note && (
        <StyledNote role="note">
          <InfoGlyph />
          <span>{note}</span>
        </StyledNote>
      )}
      {current !== 'direct' && availability === 'needs-full-disk-access' && <FullDiskAccessNote />}
    </StyledSection>
  );
}
