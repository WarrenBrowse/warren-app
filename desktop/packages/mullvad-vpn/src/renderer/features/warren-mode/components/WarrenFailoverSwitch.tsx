import { Switch, SwitchProps } from '../../../lib/components/switch';
import { useWarrenFailover } from '../hooks';

// UI toggle for the multi-exit auto-failover setting (M5.B.2).
export type WarrenFailoverSwitch = SwitchProps;

function WarrenFailoverSwitch({ children, ...props }: WarrenFailoverSwitch) {
  const { warrenFailover, setWarrenFailover } = useWarrenFailover();

  return (
    <Switch checked={warrenFailover} onCheckedChange={setWarrenFailover} {...props}>
      {children}
    </Switch>
  );
}

const WarrenFailoverSwitchNamespace = Object.assign(WarrenFailoverSwitch, {
  Label: Switch.Label,
  Input: Switch.Input,
  Trigger: Switch.Trigger,
});

export { WarrenFailoverSwitchNamespace as WarrenFailoverSwitch };
