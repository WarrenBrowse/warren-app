import { Switch, SwitchProps } from '../../../../lib/components/switch';
import { usePortForwardingNotifications } from '../../hooks';

export type PortForwardingNotificationsSwitchProps = SwitchProps;

function PortForwardingNotificationsSwitch({
  children,
  ...props
}: PortForwardingNotificationsSwitchProps) {
  const { portForwardingNotifications, setPortForwardingNotifications } =
    usePortForwardingNotifications();

  return (
    <Switch
      checked={portForwardingNotifications}
      onCheckedChange={setPortForwardingNotifications}
      {...props}>
      {children}
    </Switch>
  );
}

const PortForwardingNotificationsSwitchNamespace = Object.assign(
  PortForwardingNotificationsSwitch,
  {
    Label: Switch.Label,
    Input: Switch.Input,
    Trigger: Switch.Trigger,
  },
);

export { PortForwardingNotificationsSwitchNamespace as PortForwardingNotificationsSwitch };
