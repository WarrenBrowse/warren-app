import { Switch, SwitchProps } from '../../../lib/components/switch';
import { useWarrenMultiHop } from '../hooks';

// Toggle primitive for `Settings::warren_multi_hop.enabled`. Reuses
// the design-system `Switch`. Per doctrine `warren_multihop_doctrine_v1`
// the default is OFF (opt-in privacy, full bandwidth single-hop).
export type WarrenMultiHopSwitchProps = SwitchProps;

function WarrenMultiHopSwitch({ children, ...props }: WarrenMultiHopSwitchProps) {
  const { warrenMultiHop, setEnabled } = useWarrenMultiHop();

  return (
    <Switch checked={warrenMultiHop.enabled} onCheckedChange={setEnabled} {...props}>
      {children}
    </Switch>
  );
}

const WarrenMultiHopSwitchNamespace = Object.assign(WarrenMultiHopSwitch, {
  Label: Switch.Label,
  Input: Switch.Input,
  Trigger: Switch.Trigger,
});

export { WarrenMultiHopSwitchNamespace as WarrenMultiHopSwitch };
