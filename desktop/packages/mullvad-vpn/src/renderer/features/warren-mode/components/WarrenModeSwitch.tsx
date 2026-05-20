import { Switch, SwitchProps } from '../../../lib/components/switch';
import { useWarrenMode } from '../hooks';

// UI toggle for `Settings::warren_mode`. Reuses the `Switch` primitive
// from the design system. To be placed in a Settings page (e.g. next
// to the "Allow LAN" toggle) or in a dedicated "Warren mode" page
// once the UX is designed.
export type WarrenModeSwitch = SwitchProps;

function WarrenModeSwitch({ children, ...props }: WarrenModeSwitch) {
  const { warrenMode, setWarrenMode } = useWarrenMode();

  return (
    <Switch checked={warrenMode} onCheckedChange={setWarrenMode} {...props}>
      {children}
    </Switch>
  );
}

const WarrenModeSwitchNamespace = Object.assign(WarrenModeSwitch, {
  Label: Switch.Label,
  Input: Switch.Input,
  Trigger: Switch.Trigger,
});

export { WarrenModeSwitchNamespace as WarrenModeSwitch };
