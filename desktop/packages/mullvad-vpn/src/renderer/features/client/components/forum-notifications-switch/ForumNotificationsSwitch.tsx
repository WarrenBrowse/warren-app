import { Switch, SwitchProps } from '../../../../lib/components/switch';
import { useForumNotifications } from '../../hooks';

export type ForumNotificationsSwitchProps = SwitchProps;

function ForumNotificationsSwitch({ children, ...props }: ForumNotificationsSwitchProps) {
  const { forumNotifications, setForumNotifications } = useForumNotifications();

  return (
    <Switch checked={forumNotifications} onCheckedChange={setForumNotifications} {...props}>
      {children}
    </Switch>
  );
}

const ForumNotificationsSwitchNamespace = Object.assign(ForumNotificationsSwitch, {
  Label: Switch.Label,
  Input: Switch.Input,
  Trigger: Switch.Trigger,
});

export { ForumNotificationsSwitchNamespace as ForumNotificationsSwitch };
