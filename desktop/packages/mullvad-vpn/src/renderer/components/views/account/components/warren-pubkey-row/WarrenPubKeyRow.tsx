import { Text } from '../../../../../lib/components';
import { useSelector } from '../../../../../redux/store';
import WarrenPubKeyLabel from '../../../../WarrenPubKeyLabel';

export function WarrenPubKeyRow() {
  const pubkey = useSelector((state) => state.account.pubkey);
  return <Text variant="bodySmallSemibold" as={WarrenPubKeyLabel} pubkey={pubkey || ''} />;
}
