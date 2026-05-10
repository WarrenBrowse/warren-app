import { Switch, SwitchProps } from '../../../lib/components/switch';
import { useWarrenMode } from '../hooks';

// Toggle UI pour `Settings::warren_mode`. Réutilise le primitif
// `Switch` du design system. À placer dans une page Settings (e.g. à
// côté du toggle "Allow LAN") ou dans une page dédiée "Warren mode"
// lorsque l'UX sera maquettée.
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
