import { Switch, SwitchProps } from '../../../lib/components/switch';
import { useWarrenLocalAccount } from '../hooks';

// UI toggle for `Settings::warren_local_account`. Symmetric to
// `WarrenModeSwitch` (cf. doc).
export type WarrenLocalAccountSwitch = SwitchProps;

function WarrenLocalAccountSwitch({ children, ...props }: WarrenLocalAccountSwitch) {
  const { warrenLocalAccount, setWarrenLocalAccount } = useWarrenLocalAccount();

  return (
    <Switch checked={warrenLocalAccount} onCheckedChange={setWarrenLocalAccount} {...props}>
      {children}
    </Switch>
  );
}

const WarrenLocalAccountSwitchNamespace = Object.assign(WarrenLocalAccountSwitch, {
  Label: Switch.Label,
  Input: Switch.Input,
  Trigger: Switch.Trigger,
});

export { WarrenLocalAccountSwitchNamespace as WarrenLocalAccountSwitch };
