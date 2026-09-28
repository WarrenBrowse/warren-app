import React from 'react';
import { sprintf } from 'sprintf-js';
import styled from 'styled-components';

import { type AppRoute, appRouteFor, defaultRoute } from '../../../../../../shared/app-routing';
import { type ISplitTunnelingApplication } from '../../../../../../shared/application-types';
import { messages } from '../../../../../../shared/gettext';
import { useAppContext } from '../../../../../context';
import { CountryFlag } from '../../../../../features/app-routing/components';
import { useAppRouting, useExitChoiceNames } from '../../../../../features/app-routing/hooks';
import { routingLimitationText } from '../../../../../features/app-routing/strings';
import { Button } from '../../../../../lib/components';
import { colors } from '../../../../../lib/foundations';
import { ModalAlert, ModalAlertType } from '../../../../Modal';
import { useRoutingActions } from '../../hooks/use-routing-actions';
import { type RoutingTarget, useSplitTunnelingContext } from '../../SplitTunnelingContext';
import { useSplitModeAvailability } from '../split-tunneling-settings/hooks';
import { AppAvatar } from './AppAvatar';
import { CheckBadge, GlobeGlyph, OutsideGlyph, ShieldGlyph } from './glyphs';
import { ScreenFrame } from './ScreenFrame';
import { outsideUnavailableReason, outsideVpnLabel, throughVpnLabel } from './strings';
import {
  StyledHelp,
  StyledRowButton,
  StyledScreenBody,
  StyledScreenButton,
  StyledScreenTitle,
  StyledTextButton,
} from './styles';

const StyledTitleRow = styled.div({
  display: 'flex',
  alignItems: 'center',
  gap: '12px',
});

const StyledOption = styled(StyledRowButton)({
  gap: '14px',
  minHeight: '68px',
  padding: '10px 14px',
});

const StyledOptionText = styled.span({
  flex: 1,
  minWidth: 0,
  display: 'flex',
  flexDirection: 'column',
  gap: '2px',
});

const StyledOptionTitle = styled.span({
  fontSize: '18px',
  lineHeight: '24px',
  overflowWrap: 'anywhere',
});

const StyledOptionSubtitle = styled.span({
  fontSize: '14px',
  lineHeight: '19px',
  color: colors.whiteAlpha60,
});

const StyledBadge = styled.span({
  flexShrink: 0,
  fontWeight: 400,
  fontSize: '13px',
  lineHeight: '18px',
  color: colors.whiteOnDarkBlue60,
  border: `1.5px solid ${colors.whiteOnBlue20}`,
  borderRadius: '999px',
  padding: '2px 8px',
  whiteSpace: 'nowrap',
});

function ChevronGlyph() {
  return (
    <svg
      width="18"
      height="18"
      viewBox="0 0 24 24"
      fill="none"
      stroke={colors.whiteOnDarkBlue40}
      strokeWidth="2.5"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden
      style={{ flexShrink: 0 }}>
      <path d="M9 18l6-6-6-6" />
    </svg>
  );
}

type OptionCardProps = {
  glyph: React.ReactNode;
  title: string;
  subtitle: string;
  isDefault?: boolean;
  selected?: boolean;
  disabled?: boolean;
  opensMore?: boolean;
  testId: string;
  onClick: () => void;
};

function OptionCard(props: OptionCardProps) {
  return (
    <StyledOption
      type="button"
      data-testid={props.testId}
      $selected={props.selected}
      aria-pressed={props.opensMore ? undefined : props.selected}
      disabled={props.disabled}
      onClick={props.onClick}>
      {props.glyph}
      <StyledOptionText>
        <StyledOptionTitle>{props.title}</StyledOptionTitle>
        <StyledOptionSubtitle>{props.subtitle}</StyledOptionSubtitle>
      </StyledOptionText>
      {props.isDefault && (
        <StyledBadge>
          {
            // TRANSLATORS: Badge on the route the apps without a rule take.
            messages.pgettext('split-tunneling-view', 'Default')
          }
        </StyledBadge>
      )}
      {props.selected && <CheckBadge />}
      {props.opensMore && <ChevronGlyph />}
    </StyledOption>
  );
}

function asApplication(target: RoutingTarget): ISplitTunnelingApplication | undefined {
  return typeof target.application === 'string' ? undefined : target.application;
}

// Linux keeps no list: an app leaves or joins the VPN when Warren opens it,
// from its desktop entry when it has one.
function launchPath(target: RoutingTarget): string {
  const application = asApplication(target);
  return application?.launchPath ?? application?.absolutepath ?? target.id;
}

type LinuxLaunchProps = {
  target: RoutingTarget;
  into: 'outside' | 'vpn';
  supported: boolean;
};

function LinuxLaunchCard({ target, into, supported }: LinuxLaunchProps) {
  const { launchExcludedApplication, launchIncludedApplication } = useAppContext();
  const [error, setError] = React.useState<string>();
  const closeError = React.useCallback(() => setError(undefined), []);
  const problematic = asApplication(target)?.launchWarning === 'launches-elsewhere';

  const launch = React.useCallback(async () => {
    const run = into === 'outside' ? launchExcludedApplication : launchIncludedApplication;
    const result = await run(launchPath(target));
    if ('error' in result) {
      setError(result.error);
    }
  }, [into, launchExcludedApplication, launchIncludedApplication, target]);

  let subtitle = messages.pgettext(
    'split-tunneling-view',
    'Until you close it. Close it first if it is already open.',
  );
  if (!supported) {
    subtitle = messages.pgettext('split-tunneling-view', 'Not available on this system');
  } else if (problematic) {
    subtitle = sprintf(
      into === 'outside'
        ? messages.pgettext(
            'split-tunneling-view',
            '%(applicationName)s is problematic and can’t be excluded from the VPN tunnel.',
          )
        : messages.pgettext(
            'split-tunneling-view',
            '%(applicationName)s is problematic and can’t be launched through the VPN alone.',
          ),
      { applicationName: target.name },
    );
  }

  return (
    <>
      <OptionCard
        testId={into === 'outside' ? 'route-open-outside' : 'route-open-vpn'}
        glyph={into === 'outside' ? <OutsideGlyph size={26} /> : <ShieldGlyph size={26} />}
        title={
          into === 'outside'
            ? // TRANSLATORS: Linux: opens the app now, outside the VPN.
              messages.pgettext('split-tunneling-view', 'Open outside the VPN')
            : // TRANSLATORS: Linux: opens the app now, through the VPN.
              messages.pgettext('split-tunneling-view', 'Open through the VPN')
        }
        subtitle={subtitle}
        disabled={!supported || problematic}
        onClick={launch}
      />
      <ModalAlert
        isOpen={error !== undefined}
        type={ModalAlertType.warning}
        iconColor={colors.red}
        message={sprintf(
          // TRANSLATORS: Error message showed in a dialog when an application fails to launch.
          messages.pgettext(
            'split-tunneling-view',
            'Unable to launch selection. %(detailedErrorMessage)s',
          ),
          { detailedErrorMessage: error ?? '' },
        )}
        buttons={[
          <Button key="close" onClick={closeError}>
            <Button.Text>{messages.gettext('Close')}</Button.Text>
          </Button>,
        ]}
        close={closeError}
      />
    </>
  );
}

// The route of one app: through the VPN, through the VPN from another
// country, or outside the VPN. The option equal to the default is the absence
// of a rule.
export function RouteScreen() {
  const { routing, platform } = useAppRouting();
  const { target, showList, showCountry } = useSplitTunnelingContext();
  const { setAppRoute } = useRoutingActions();
  const availability = useSplitModeAvailability();
  const exitNames = useExitChoiceNames();
  const [busy, setBusy] = React.useState(false);

  const choose = React.useCallback(
    async (route: AppRoute, leave = false) => {
      if (target === undefined) return;
      setBusy(true);
      await setAppRoute(target, route);
      setBusy(false);
      // A picked program is resolved by the main process to the id the
      // daemon keys, which this screen cannot follow.
      if (leave || typeof target.application === 'string') {
        showList();
      }
    },
    [setAppRoute, showList, target],
  );
  const chooseVpn = React.useCallback(() => void choose({ kind: 'vpn' }), [choose]);
  const chooseOutside = React.useCallback(() => void choose({ kind: 'direct' }), [choose]);

  const fallback = defaultRoute(routing);
  const removeRule = React.useCallback(
    () => void choose({ kind: fallback }, true),
    [choose, fallback],
  );

  if (target === undefined) {
    return null;
  }

  const current = appRouteFor(routing, target.id, platform);
  const hasRule = current.kind !== fallback;
  const linux = platform === 'linux';
  const limitation = asApplication(target)?.routingLimitation;
  const outsideReason =
    fallback === 'vpn' && current.kind !== 'direct'
      ? outsideUnavailableReason(availability)
      : undefined;

  const exit = current.kind === 'country' ? current.exit : undefined;
  const place = exit ? exitNames(exit) : undefined;
  const placeLabel = place
    ? place.city === undefined
      ? place.country
      : `${place.city}, ${place.country}`
    : undefined;

  return (
    <ScreenFrame testId="route-screen" onClose={showList}>
      <StyledTitleRow>
        <AppAvatar name={target.name} icon={asApplication(target)?.icon} size={44} />
        <StyledScreenTitle>
          {sprintf(
            // TRANSLATORS: Title of the screen choosing how one app connects.
            // TRANSLATORS: Available placeholders:
            // TRANSLATORS: %(application)s - the app's name
            messages.pgettext('split-tunneling-view', 'Route for %(application)s'),
            { application: target.name },
          )}
        </StyledScreenTitle>
      </StyledTitleRow>

      <StyledScreenBody>
        {linux && fallback === 'direct' ? (
          <LinuxLaunchCard target={target} into="vpn" supported={availability === 'available'} />
        ) : (
          <OptionCard
            testId="route-vpn"
            glyph={<ShieldGlyph size={26} />}
            title={throughVpnLabel()}
            subtitle={messages.pgettext('split-tunneling-view', 'Country of your main connection')}
            isDefault={fallback === 'vpn'}
            selected={current.kind === 'vpn'}
            disabled={busy}
            onClick={chooseVpn}
          />
        )}

        <OptionCard
          testId="route-country"
          glyph={exit ? <CountryFlag country={exit.country} size={26} /> : <GlobeGlyph />}
          title={
            placeLabel ??
            messages.pgettext('split-tunneling-view', 'Through the VPN, another country')
          }
          subtitle={
            limitation
              ? routingLimitationText(limitation)
              : exit
                ? messages.pgettext('split-tunneling-view', 'Through the VPN, from this country')
                : messages.pgettext('split-tunneling-view', 'Choose an exit country')
          }
          selected={exit !== undefined}
          disabled={busy || limitation !== undefined}
          opensMore
          onClick={showCountry}
        />

        {linux && fallback === 'vpn' ? (
          <LinuxLaunchCard
            target={target}
            into="outside"
            supported={availability === 'available'}
          />
        ) : (
          <OptionCard
            testId="route-outside"
            glyph={<OutsideGlyph size={26} />}
            title={outsideVpnLabel()}
            subtitle={
              outsideReason ??
              messages.pgettext(
                'split-tunneling-view',
                'Direct connection, your real IP is visible',
              )
            }
            isDefault={fallback === 'direct'}
            selected={current.kind === 'direct'}
            disabled={busy || outsideReason !== undefined}
            onClick={chooseOutside}
          />
        )}

        {linux && fallback === 'direct' && exit !== undefined && (
          <StyledHelp role="note">
            {messages.pgettext(
              'split-tunneling-view',
              'It uses this country when you open it with “Open through the VPN”.',
            )}
          </StyledHelp>
        )}
      </StyledScreenBody>

      {hasRule && (
        <StyledTextButton type="button" disabled={busy} onClick={removeRule}>
          {
            // TRANSLATORS: Button giving an app back the default route.
            messages.pgettext('split-tunneling-view', 'Remove the rule')
          }
        </StyledTextButton>
      )}
      <StyledScreenButton type="button" onClick={showList}>
        {
          // TRANSLATORS: Button closing the route of an app.
          messages.pgettext('split-tunneling-view', 'Done')
        }
      </StyledScreenButton>
    </ScreenFrame>
  );
}
