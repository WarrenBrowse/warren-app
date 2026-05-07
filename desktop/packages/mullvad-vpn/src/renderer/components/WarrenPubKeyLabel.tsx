import { formatWarrenPubKey } from '../lib/pubkey';
import ClipboardLabel from './ClipboardLabel';

interface IWarrenPubKeyLabelProps {
  pubkey: string;
  obscureValue?: boolean;
  className?: string;
}

export default function WarrenPubKeyLabel(props: IWarrenPubKeyLabelProps) {
  return (
    <ClipboardLabel
      value={props.pubkey}
      displayValue={formatWarrenPubKey(props.pubkey)}
      obscureValue={props.obscureValue}
      className={props.className}
      data-testid="warren-pubkey"
    />
  );
}
