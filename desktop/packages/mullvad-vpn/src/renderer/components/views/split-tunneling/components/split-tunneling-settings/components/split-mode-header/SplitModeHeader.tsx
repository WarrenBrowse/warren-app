import React from 'react';
import styled from 'styled-components';

import { messages } from '../../../../../../../../shared/gettext';
import { useAppRouting } from '../../../../../../../features/app-routing/hooks';
import { splitModeUnavailableText } from '../../../../../../../features/app-routing/strings';
import { Icon } from '../../../../../../../lib/components';
import { spacings } from '../../../../../../../lib/foundations';
import { HeaderSubTitle } from '../../../../../../SettingsHeader';
import { useSplitTunnelingContext } from '../../../../SplitTunnelingContext';
import { tabLabel } from '../../../app-routing-tabs';
import { IncludeOnlyBanner } from '../../../include-only-banner';
import { TabHeader } from '../../../tab-header';
import { useSplitModeAvailability } from '../../hooks';
import { MacOsSplitTunnelingAvailability } from '../macos-split-tunneling-availability';

const StyledUnavailable = styled(HeaderSubTitle)({
  display: 'flex',
  gap: spacings.tiny,
  alignItems: 'center',
});

export type SplitModeHeaderProps = {
  mode: 'exclude' | 'include-only';
};

// The switch of Bypass VPN or VPN only for, with what stands in its way on
// this device. Turning one on goes through the confirmation the view holds.
export function SplitModeHeader({ mode }: SplitModeHeaderProps) {
  const { routing } = useAppRouting();
  const { requestSplitMode } = useSplitTunnelingContext();
  const availability = useSplitModeAvailability();

  const checked = routing.splitMode === mode;
  const setChecked = React.useCallback(
    (value: boolean) => requestSplitMode(value ? mode : 'off'),
    [mode, requestSplitMode],
  );
  // Turning a mode off is never refused, whatever the device can do.
  const disabled = !checked && availability !== 'available';
  const unavailableText = splitModeUnavailableText(availability);

  const description =
    mode === 'exclude'
      ? messages.pgettext(
          'split-tunneling-view',
          'The apps you choose connect as if the VPN were off.',
        )
      : messages.pgettext('split-tunneling-view', 'Only the apps you choose use the VPN.');

  return (
    <TabHeader
      label={tabLabel(mode === 'exclude' ? 'bypass' : 'include-only')}
      description={description}
      checked={checked}
      disabled={disabled}
      onCheckedChange={setChecked}>
      {availability === 'needs-full-disk-access' && <MacOsSplitTunnelingAvailability />}
      {unavailableText && (
        <StyledUnavailable role="note">
          <Icon icon="info-circle" size="small" color="whiteAlpha60" aria-hidden />
          {unavailableText}
        </StyledUnavailable>
      )}
      {mode === 'include-only' && checked && <IncludeOnlyBanner />}
    </TabHeader>
  );
}
