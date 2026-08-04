import { Switch, SwitchProps } from '../../../../lib/components/switch';
import { usePortForwarding } from '../../hooks';

export type PortForwardingSwitchProps = SwitchProps;

function PortForwardingSwitch({ children, ...props }: PortForwardingSwitchProps) {
  const { settings, setEnabled } = usePortForwarding();

  return (
    <Switch checked={settings.enabled} onCheckedChange={setEnabled} {...props}>
      {children}
    </Switch>
  );
}

const PortForwardingSwitchNamespace = Object.assign(PortForwardingSwitch, {
  Label: Switch.Label,
  Input: Switch.Input,
  Trigger: Switch.Trigger,
});

export { PortForwardingSwitchNamespace as PortForwardingSwitch };
