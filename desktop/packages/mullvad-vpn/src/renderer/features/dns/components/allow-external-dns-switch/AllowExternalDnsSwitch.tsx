import { Switch, SwitchProps } from '../../../../lib/components/switch';
import { useDns, useSetAllowExternalDns } from '../../hooks';

export type AllowExternalDnsSwitchProps = SwitchProps;

function AllowExternalDnsSwitch({ children, ...props }: AllowExternalDnsSwitchProps) {
  const { dns } = useDns();
  const setAllowExternalDns = useSetAllowExternalDns();

  return (
    <Switch checked={dns.allowExternalDns} onCheckedChange={setAllowExternalDns} {...props}>
      {children}
    </Switch>
  );
}

const AllowExternalDnsSwitchNamespace = Object.assign(AllowExternalDnsSwitch, {
  Label: Switch.Label,
  Input: Switch.Input,
  Trigger: Switch.Trigger,
});

export { AllowExternalDnsSwitchNamespace as AllowExternalDnsSwitch };
