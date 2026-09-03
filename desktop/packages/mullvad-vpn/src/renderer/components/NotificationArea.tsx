import { AnimatePresence } from 'motion/react';
import { useCallback, useState } from 'react';

import { urls } from '../../shared/constants';
import { messages } from '../../shared/gettext';
import log from '../../shared/logging';
import {
  CloseToAccountExpiryNotificationProvider,
  ConnectingNotificationProvider,
  ErrorNotificationProvider,
  InAppNotificationAction,
  InAppNotificationProvider,
  InconsistentVersionNotificationProvider,
  LockdownModeNotificationProvider,
  ReconnectingNotificationProvider,
  RenewalUpcomingInAppNotificationProvider,
  UnsupportedVersionNotificationProvider,
} from '../../shared/notifications';
import { useAppContext } from '../context';
import { useSplitTunnelingSupported } from '../features/split-tunneling/hooks';
import {
  useAppUpgradeDownloadProgressValue,
  useAppUpgradeEventType,
  useHasAppUpgradeError,
} from '../hooks';
import { Button } from '../lib/components';
import { useConnectingStuck } from '../lib/connecting-stuck';
import { useExitEgressDead } from '../lib/exit-egress';
import { useHostOffline } from '../lib/host-offline';
import {
  AppUpgradeErrorNotificationProvider,
  AppUpgradeProgressNotificationProvider,
  AppUpgradeReadyNotificationProvider,
  NewVersionNotificationProvider,
  WarrenConnectingStuckNotificationProvider,
  WarrenExitEgressNotificationProvider,
  WarrenFailoverNotificationProvider,
  WarrenHostOfflineNotificationProvider,
  WarrenMaintenanceNotificationProvider,
  WarrenNoticeNotificationProvider,
  WarrenPortMigrationNotificationProvider,
  WarrenUncleanRestoreNotificationProvider,
} from '../lib/notifications';
import { AppUpgradeAvailableNotificationProvider } from '../lib/notifications/app-upgrade-available';
import { useMounted } from '../lib/utility-hooks';
import { convertEventTypeToStep } from '../redux/app-upgrade/helpers';
import { useAppUpgradeError, useVersionSuggestedUpgrade } from '../redux/hooks';
import { IReduxState, useSelector } from '../redux/store';
import { ModalAlert, ModalAlertType, ModalMessage, ModalMessageList } from './Modal';
import {
  NotificationActions,
  NotificationBanner,
  NotificationCloseAction,
  NotificationContent,
  NotificationExpandableSubtitle,
  NotificationIndicator,
  NotificationOpenLinkAction,
  NotificationTitle,
  NotificationTroubleshootDialogAction,
} from './NotificationBanner';
import { NotificationSubtitle } from './NotificationSubtitle';

interface IProps {
  className?: string;
}

export default function NotificationArea(props: IProps) {
  const { showFullDiskAccessSettings } = useAppContext();

  const account = useSelector((state: IReduxState) => state.account);
  const locale = useSelector((state: IReduxState) => state.userInterface.locale);
  const tunnelState = useSelector((state: IReduxState) => state.connection.status);
  const version = useSelector((state: IReduxState) => state.version);

  const lockdownModeSetting = useSelector((state: IReduxState) => state.settings.lockdownMode);
  const hasExcludedApps = useSelector(
    (state: IReduxState) =>
      state.settings.splitTunneling && state.settings.splitTunnelingApplications.length > 0,
  );

  const { setDisplayedChangelog, setDismissedUpgrade, appUpgrade, appUpgradeInstallerStart } =
    useAppContext();

  const currentVersion = useSelector((state) => state.version.current);
  const displayedForVersion = useSelector(
    (state) => state.settings.guiSettings.changelogDisplayedForVersion,
  );
  const changelog = useSelector((state) => state.userInterface.changelog);

  const close = useCallback(() => {
    setDisplayedChangelog();
  }, [setDisplayedChangelog]);

  const [isModalOpen, setIsModalOpen] = useState(false);
  // Full text of a clamped banner, shown in a scrollable modal. Held here
  // rather than in the provider because the providers are pure data.
  const [expandedText, setExpandedText] = useState<{ title: string; content: string }>();

  // Failover banner state. Local to the renderer process: the
  // daemon counter resets on restart, and so should the
  // "acknowledged" mark (so a fresh boot's first failover always
  // surfaces). The banner stays visible while
  // `warrenStatus.failoverCount > warrenFailoverAcknowledged`.
  const [warrenFailoverAcknowledged, setWarrenFailoverAcknowledged] = useState(0);
  const warrenStatus = useSelector((state: IReduxState) => state.settings.warrenStatus);
  const hostOffline = useHostOffline();
  const connectingStuck = useConnectingStuck();
  const exitEgressDead = useExitEgressDead();
  const acknowledgeWarrenFailover = useCallback(() => {
    setWarrenFailoverAcknowledged(warrenStatus?.failoverCount ?? 0);
  }, [warrenStatus?.failoverCount]);

  const { setSplitTunnelingState } = useAppContext();
  const disableSplitTunneling = useCallback(async () => {
    setIsModalOpen(false);
    await setSplitTunnelingState(false);
  }, [setSplitTunnelingState]);

  const updateDismissedForVersion = useSelector(
    (state) => state.settings.guiSettings.updateDismissedForVersion,
  );
  const hasAppUpgradeError = useHasAppUpgradeError();
  const { error } = useAppUpgradeError();

  const restartAppUpgrade = useCallback(() => {
    appUpgrade();
  }, [appUpgrade]);
  const restartAppUpgradeInstaller = useCallback(() => {
    appUpgradeInstallerStart();
  }, [appUpgradeInstallerStart]);

  const { suggestedUpgrade } = useVersionSuggestedUpgrade();

  const { splitTunnelingSupported } = useSplitTunnelingSupported();

  const appUpgradeDownloadProgressValue = useAppUpgradeDownloadProgressValue();
  const appUpgradeEventType = useAppUpgradeEventType();
  const appUpgradeStep = convertEventTypeToStep(appUpgradeEventType);

  const notificationProviders: InAppNotificationProvider[] = [
    // Above everything, including the network-loss banner: when the
    // operator has published a notice, that message is the one thing the
    // user must read. The states it covers are still legible in the
    // connect view's own status, and the daemon clears the notice (empty
    // list) as soon as it is erased or lapses.
    new WarrenNoticeNotificationProvider({ notices: warrenStatus?.notices ?? [] }),
    // Then: losing the network trumps every other banner, and the flag
    // fires on the daemon's offline edge (debounced only past the
    // synthetic ~1 s blips), before the tunnel state machine reacts (it
    // holds Connected through its migration grace window).
    new WarrenHostOfflineNotificationProvider({
      hostOffline: hostOffline,
    }),
    // Same rank family, distinct cause: the host network is fine but
    // the exit stopped forwarding (doc 62 item 5). Only meaningful
    // while the tunnel still claims Connected.
    new WarrenExitEgressNotificationProvider({
      exitEgressDead,
      tunnelConnected: tunnelState.state === 'connected',
    }),
    // Before the plain Connecting banner so a stalled attempt upgrades
    // the banner to the forum help hint instead of spinning forever.
    new WarrenConnectingStuckNotificationProvider({ connectingStuck }),
    new ConnectingNotificationProvider({ tunnelState }),
    new ReconnectingNotificationProvider(tunnelState),
    new LockdownModeNotificationProvider({
      tunnelState,
      lockdownModeSetting,
      hasExcludedApps,
    }),
    new AppUpgradeErrorNotificationProvider({
      hasAppUpgradeError,
      appUpgradeError: error,
      restartAppUpgrade,
      restartAppUpgradeInstaller,
    }),
    new AppUpgradeReadyNotificationProvider({
      appUpgradeEventType,
      suggestedUpgradeVersion: suggestedUpgrade?.version,
    }),
    new AppUpgradeProgressNotificationProvider({
      appUpgradeStep,
      appUpgradeEventType,
      appUpgradeDownloadProgressValue,
    }),
    new ErrorNotificationProvider({
      tunnelState,
      hasExcludedApps,
      showFullDiskAccessSettings,
      disableSplitTunneling,
      splitTunnelingSupported,
    }),
    new InconsistentVersionNotificationProvider({ consistent: version.consistent }),
    new UnsupportedVersionNotificationProvider(version),
    new WarrenFailoverNotificationProvider({
      failoverCount: warrenStatus?.failoverCount ?? 0,
      acknowledgedCount: warrenFailoverAcknowledged,
      close: acknowledgeWarrenFailover,
    }),
    new WarrenMaintenanceNotificationProvider({
      maintenanceMigrationActive: warrenStatus?.maintenanceMigrationActive ?? false,
    }),
    new WarrenPortMigrationNotificationProvider({
      portMigrationCancellationActive: warrenStatus?.portMigrationCancellationActive ?? false,
    }),
    new WarrenUncleanRestoreNotificationProvider({
      restoredAfterUncleanShutdown: warrenStatus?.restoredAfterUncleanShutdown ?? false,
    }),
  ];

  if (account.expiry) {
    notificationProviders.push(
      // Before the expiry alarm: when auto-renewal is armed, "expires
      // soon, buy more" is the wrong message and this banner is the
      // in-window half of the legal pre-charge notice.
      new RenewalUpcomingInAppNotificationProvider({
        accountExpiry: account.expiry,
        renewalActive: account.renewalState !== undefined,
      }),
      new CloseToAccountExpiryNotificationProvider({ accountExpiry: account.expiry, locale }),
    );
  }

  notificationProviders.push(
    new NewVersionNotificationProvider({
      currentVersion,
      displayedForVersion,
      changelog,
      close,
    }),
    new AppUpgradeAvailableNotificationProvider({
      platform: window.env.platform,
      suggestedUpgradeVersion: suggestedUpgrade?.version,
      suggestedIsBeta: version.suggestedIsBeta,
      updateDismissedForVersion,
      close: setDismissedUpgrade,
    }),
  );

  const notificationProvider = notificationProviders.find((notification) =>
    notification.mayDisplay(),
  );

  const notification = notificationProvider?.getInAppNotification();
  if (notificationProvider) {
    if (!notification) {
      log.error(
        `Notification providers mayDisplay() returned true but getInAppNotification() returned undefined for ${notificationProvider.constructor.name}`,
      );
    }
  }

  // We only want to animate notifications after first mount,
  // so as to prevent an animation from animating in when the
  // app has just started.
  const mounted = useMounted();
  const isMounted = mounted();

  const expandable =
    notification?.action?.type === 'expand-text' ? notification.action.expand : undefined;
  const showFullText = useCallback(() => setExpandedText(expandable), [expandable]);
  const hideFullText = useCallback(() => setExpandedText(undefined), []);

  return (
    <AnimatePresence>
      {notification && (
        <NotificationBanner
          animateIn={isMounted}
          aria-hidden={!notification}
          className={props.className}>
          <NotificationIndicator
            $type={notification.indicator}
            data-testid="notificationIndicator"
          />
          <NotificationContent role="status" aria-live="polite">
            <NotificationTitle data-testid="notificationTitle">
              {notification.title}
            </NotificationTitle>
            {expandable ? (
              <NotificationExpandableSubtitle
                text={expandable.content}
                expandLabel={
                  // TRANSLATORS: Shown under a banner whose text was too long
                  // TRANSLATORS: to fit; opens the full message.
                  messages.pgettext('in-app-notifications', 'Read the full message')
                }
                onExpand={showFullText}
              />
            ) : (
              <NotificationSubtitle
                data-testid="notificationSubTitle"
                subtitle={notification.subtitle}
              />
            )}
          </NotificationContent>
          {notification.action && (
            <NotificationActionWrapper
              action={notification.action}
              isModalOpen={isModalOpen}
              setIsModalOpen={setIsModalOpen}
            />
          )}
          <ModalAlert
            isOpen={expandedText !== undefined}
            type={ModalAlertType.info}
            title={expandedText?.title}
            close={hideFullText}
            buttons={[
              <Button key="close" onClick={hideFullText}>
                <Button.Text>{messages.gettext('Close')}</Button.Text>
              </Button>,
            ]}>
            <ModalMessage>{expandedText?.content}</ModalMessage>
          </ModalAlert>
        </NotificationBanner>
      )}
    </AnimatePresence>
  );
}

interface NotificationActionWrapperProps {
  action: InAppNotificationAction;
  isModalOpen: boolean;
  setIsModalOpen: (isOpen: boolean) => void;
}

function NotificationActionWrapper({
  action,
  isModalOpen,
  setIsModalOpen,
}: NotificationActionWrapperProps) {
  const { openUrl } = useAppContext();

  const closeTroubleshootModal = useCallback(() => setIsModalOpen(false), [setIsModalOpen]);

  const handleClick = useCallback(() => {
    if (action) {
      switch (action.type) {
        case 'navigate-external':
          return openUrl(action.link.to);
        case 'troubleshoot-dialog':
          setIsModalOpen(true);
          break;
        case 'close':
          action.close();
          break;
      }
    }

    return Promise.resolve();
  }, [action, setIsModalOpen, openUrl]);

  const openForum = useCallback(() => {
    closeTroubleshootModal();
    void openUrl(urls.forum);
  }, [closeTroubleshootModal, openUrl]);

  let actionComponent: React.ReactElement | undefined;
  if (action) {
    switch (action.type) {
      case 'navigate-external':
        actionComponent = <NotificationOpenLinkAction onClick={handleClick} />;
        break;
      case 'troubleshoot-dialog':
        actionComponent = (
          <>
            <NotificationTroubleshootDialogAction onClick={handleClick} />
          </>
        );
        break;
      case 'close':
        actionComponent = <NotificationCloseAction onClick={handleClick} />;
    }
  }

  if (action.type === 'expand-text') {
    // The control lives under the text itself, so the action column would
    // only add an empty box next to it.
    return null;
  }

  if (action.type !== 'troubleshoot-dialog') {
    return <NotificationActions>{actionComponent}</NotificationActions>;
  }

  const forumButton = action.troubleshoot?.buttons ? (
    <Button key="forum" onClick={openForum}>
      <Button.Text>
        {
          // TRANSLATORS: Button label opening the community forum to report a problem.
          messages.pgettext('in-app-notifications', 'Report on the forum')
        }
      </Button.Text>
    </Button>
  ) : (
    <Button variant="success" key="forum" onClick={openForum}>
      <Button.Text>
        {
          // TRANSLATORS: Button label opening the community forum to report a problem.
          messages.pgettext('in-app-notifications', 'Report on the forum')
        }
      </Button.Text>
    </Button>
  );

  let buttons = [
    forumButton,
    <Button key="back" onClick={closeTroubleshootModal}>
      <Button.Text>{messages.gettext('Back')}</Button.Text>
    </Button>,
  ];

  if (action.troubleshoot?.buttons) {
    const actionButtons = action.troubleshoot.buttons.map(({ variant, label, action }) => (
      <Button key={label} variant={variant} onClick={action}>
        <Button.Text>{label}</Button.Text>
      </Button>
    ));

    buttons = actionButtons.concat(buttons);
  }

  return (
    <>
      <NotificationActions>{actionComponent}</NotificationActions>
      <ModalAlert
        isOpen={isModalOpen}
        type={ModalAlertType.info}
        buttons={buttons}
        close={closeTroubleshootModal}>
        <ModalMessage>{action.troubleshoot?.details}</ModalMessage>
        <ModalMessage>
          <ModalMessageList>
            {action.troubleshoot?.steps.map((step) => <li key={step}>{step}</li>)}
          </ModalMessageList>
        </ModalMessage>
        <ModalMessage>
          {messages.pgettext(
            'troubleshoot',
            'If these steps do not work, please report the problem on our community forum.',
          )}
        </ModalMessage>
      </ModalAlert>
    </>
  );
}
