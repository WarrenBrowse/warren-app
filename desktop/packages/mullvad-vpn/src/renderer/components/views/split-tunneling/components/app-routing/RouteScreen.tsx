import React from 'react';
import { sprintf } from 'sprintf-js';
import styled from 'styled-components';

import {
  appLockSupported,
  type AppRoute,
  appRouteFor,
  defaultRoute,
  isAppLocked,
} from '../../../../../../shared/app-routing';
import { type ISplitTunnelingApplication } from '../../../../../../shared/application-types';
import { messages } from '../../../../../../shared/gettext';
import { useAppContext } from '../../../../../context';
import { CountryFlag, LockGlyph } from '../../../../../features/app-routing/components';
import { useAppRouting, useExitChoiceNames } from '../../../../../features/app-routing/hooks';
import { routingLimitationText } from '../../../../../features/app-routing/strings';
import { Button } from '../../../../../lib/components';
import { Switch } from '../../../../../lib/components/switch';
import { colors } from '../../../../../lib/foundations';
import { useSelector } from '../../../../../redux/store';
import { ModalAlert, ModalAlertType } from '../../../../Modal';
import { useRoutingActions } from '../../hooks/use-routing-actions';
import { type RoutingTarget, useSplitTunnelingContext } from '../../SplitTunnelingContext';
import { useSplitModeAvailability } from '../split-tunneling-settings/hooks';
import { AppAvatar } from './AppAvatar';
import { CheckBadge, GlobeGlyph, OutsideGlyph, ShieldGlyph } from './glyphs';
import { ScreenFrame } from './ScreenFrame';
import {
  neverWithoutVpnLabel,
  outsideUnavailableReason,
  outsideVpnLabel,
  throughVpnLabel,
} from './strings';
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

const StyledOptionSubtitle = styled.span<{ $warns?: boolean }>((props) => ({
  fontSize: '14px',
  lineHeight: '19px',
  color: props.$warns ? colors.nose : colors.whiteAlpha60,
}));

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
  warns?: boolean;
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
        <StyledOptionSubtitle $warns={props.warns}>{props.subtitle}</StyledOptionSubtitle>
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

// Set apart from the three routes above it: it is not a fourth one.
const StyledLockCard = styled.div({
  marginTop: '8px',
  display: 'flex',
  alignItems: 'center',
  gap: '14px',
  minHeight: '68px',
  padding: '10px 14px',
  borderRadius: '10px',
  backgroundColor: colors.blue40,
});

// As light as the titles of the routes, which the switch label would bold.
const StyledLockTitle = styled(StyledOptionTitle)({
  fontWeight: 400,
});

const StyledLockText = styled.span({
  flex: 1,
  minWidth: 0,
  display: 'flex',
  flexDirection: 'column',
  gap: '2px',
});

type LockCardProps = {
  applicationName: string;
  locked: boolean;
  // Why the switch cannot be turned on here, if it cannot.
  unavailableReason?: string;
  busy: boolean;
  onChange: (locked: boolean) => void;
};

// "Never without the VPN": a property of the route, not a route of its own,
// so it sits apart from the three options and combines with a country.
function LockCard(props: LockCardProps) {
  const descriptionId = React.useId();
  const lockdownMode = useSelector((state) => state.settings.lockdownMode);

  let description = sprintf(
    // TRANSLATORS: Under the "Never without the VPN" switch of an app.
    // TRANSLATORS: Available placeholders:
    // TRANSLATORS: %(application)s - the app's name
    messages.pgettext(
      'split-tunneling-view',
      'When the VPN is off, %(application)s has no Internet, even with Warren VPN closed.',
    ),
    { application: props.applicationName },
  );
  if (props.unavailableReason !== undefined) {
    description = props.unavailableReason;
  } else if (lockdownMode) {
    description = messages.pgettext(
      'split-tunneling-view',
      'Lockdown mode already blocks every app while the VPN is off.',
    );
  }

  // A lock that is on can always be lifted, whatever else stopped applying.
  const disabled = props.busy || (props.unavailableReason !== undefined && !props.locked);

  return (
    <StyledLockCard data-testid="route-lock">
      <LockGlyph size={26} color={props.locked ? colors.greenText : colors.whiteOnDarkBlue40} />
      <Switch
        checked={props.locked}
        onCheckedChange={props.onChange}
        descriptionId={descriptionId}
        disabled={disabled}>
        <StyledLockText>
          <Switch.Label>
            <StyledLockTitle>{neverWithoutVpnLabel()}</StyledLockTitle>
          </Switch.Label>
          <StyledOptionSubtitle id={descriptionId}>{description}</StyledOptionSubtitle>
        </StyledLockText>
        <Switch.Input data-testid="route-lock-switch" />
      </Switch>
    </StyledLockCard>
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
  into: 'outside' | 'vpn' | 'locked';
  supported: boolean;
};

// What each way of opening an app is called and says when it cannot run.
function linuxLaunchCopy(into: LinuxLaunchProps['into']) {
  switch (into) {
    case 'outside':
      return {
        // TRANSLATORS: Linux: opens the app now, outside the VPN.
        title: messages.pgettext('split-tunneling-view', 'Open outside the VPN'),
        subtitle: messages.pgettext(
          'split-tunneling-view',
          'Until you close it. Close it first if it is already open.',
        ),
        alreadyRunning: messages.pgettext(
          'split-tunneling-view',
          'If it’s already running, close %(applicationName)s before launching it from here. Otherwise it might not be excluded from the VPN tunnel.',
        ),
        problematic: messages.pgettext(
          'split-tunneling-view',
          '%(applicationName)s is problematic and can’t be excluded from the VPN tunnel.',
        ),
      };
    case 'vpn':
      return {
        // TRANSLATORS: Linux: opens the app now, through the VPN.
        title: messages.pgettext('split-tunneling-view', 'Open through the VPN'),
        subtitle: messages.pgettext(
          'split-tunneling-view',
          'Until you close it. Close it first if it is already open.',
        ),
        alreadyRunning: messages.pgettext(
          'split-tunneling-view',
          'If it’s already running, close %(applicationName)s before launching it from here. Otherwise it might not use the VPN.',
        ),
        problematic: messages.pgettext(
          'split-tunneling-view',
          '%(applicationName)s is problematic and can’t be launched through the VPN alone.',
        ),
      };
    case 'locked':
      return {
        // TRANSLATORS: Linux: opens the app now, blocked whenever the VPN is
        // TRANSLATORS: off ("Never without the VPN" on the other platforms).
        title: messages.pgettext('split-tunneling-view', 'Open never without the VPN'),
        subtitle: messages.pgettext(
          'split-tunneling-view',
          'Until you close it, it has no Internet while the VPN is off. Close it first if it is already open.',
        ),
        alreadyRunning: messages.pgettext(
          'split-tunneling-view',
          'If it’s already running, close %(applicationName)s before launching it from here. Otherwise it might not be locked to the VPN.',
        ),
        problematic: messages.pgettext(
          'split-tunneling-view',
          '%(applicationName)s is problematic and can’t be opened locked to the VPN.',
        ),
      };
  }
}

function LinuxLaunchCard({ target, into, supported }: LinuxLaunchProps) {
  const { launchExcludedApplication, launchIncludedApplication, launchLockedApplication } =
    useAppContext();
  const [error, setError] = React.useState<string>();
  const closeError = React.useCallback(() => setError(undefined), []);
  const warning = asApplication(target)?.launchWarning;
  const limitation = asApplication(target)?.routingLimitation;
  // Flatpak and Snap start the app in a cgroup of their own, out of the one
  // Warren opened it in, so a lock would not hold.
  const sandboxed = into === 'locked' && (limitation === 'flatpak' || limitation === 'snap');
  const problematic = warning === 'launches-elsewhere';
  const copy = linuxLaunchCopy(into);

  const launch = React.useCallback(async () => {
    const run =
      into === 'outside'
        ? launchExcludedApplication
        : into === 'locked'
          ? launchLockedApplication
          : launchIncludedApplication;
    const result = await run(launchPath(target));
    if ('error' in result) {
      setError(result.error);
    }
  }, [into, launchExcludedApplication, launchIncludedApplication, launchLockedApplication, target]);

  let subtitle = copy.subtitle;
  let warns = false;
  if (!supported) {
    subtitle = messages.pgettext('split-tunneling-view', 'Not available on this system');
  } else if (sandboxed) {
    subtitle = messages.pgettext(
      'split-tunneling-view',
      'Flatpak and Snap apps cannot be opened locked to the VPN',
    );
  } else if (warning === 'launches-in-existing-process') {
    // A browser that is already open takes the new window into its running
    // process, which stays where it was: say it plainly, in the warning colour.
    warns = true;
    subtitle = sprintf(copy.alreadyRunning, { applicationName: target.name });
  } else if (problematic) {
    subtitle = sprintf(copy.problematic, { applicationName: target.name });
  }

  const glyph =
    into === 'outside' ? (
      <OutsideGlyph size={26} />
    ) : into === 'locked' ? (
      <LockGlyph size={26} />
    ) : (
      <ShieldGlyph size={26} />
    );

  return (
    <>
      <OptionCard
        testId={`route-open-${into === 'vpn' ? 'vpn' : into}`}
        glyph={glyph}
        title={copy.title}
        subtitle={subtitle}
        warns={warns}
        disabled={!supported || problematic || sandboxed}
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
  const { setAppRoute, setAppLocked } = useRoutingActions();
  const availability = useSplitModeAvailability();
  const exitNames = useExitChoiceNames();
  // The options wait for the daemon's answer: a second choice planned from
  // the state the first one is changing could undo half of it.
  const [busy, setBusy] = React.useState(false);
  const pending = React.useRef(false);

  const choose = React.useCallback(
    async (route: AppRoute, leave = false) => {
      if (target === undefined || pending.current) return;
      pending.current = true;
      setBusy(true);
      await setAppRoute(target, route);
      pending.current = false;
      setBusy(false);
      if (leave) {
        showList();
      }
    },
    [setAppRoute, showList, target],
  );
  const setLocked = React.useCallback(
    async (locked: boolean) => {
      if (target === undefined || pending.current) return;
      pending.current = true;
      setBusy(true);
      await setAppLocked(target, locked);
      pending.current = false;
      setBusy(false);
    },
    [setAppLocked, target],
  );
  const changeLock = React.useCallback((locked: boolean) => void setLocked(locked), [setLocked]);
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
  const locked = isAppLocked(routing, target.id, platform);
  const hasRule = current.kind !== fallback || locked;
  const lockReason =
    current.kind === 'direct'
      ? messages.pgettext('split-tunneling-view', 'Choose a route through the VPN to turn this on')
      : undefined;
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

        {linux && (
          <LinuxLaunchCard target={target} into="locked" supported={availability === 'available'} />
        )}

        {appLockSupported(platform) && (
          <LockCard
            applicationName={target.name}
            locked={locked}
            unavailableReason={lockReason}
            busy={busy}
            onChange={changeLock}
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
