import { Text } from '../../../../../lib/components';
import { useSelector } from '../../../../../redux/store';
import AccountNumberLabel from '../../../../AccountNumberLabel';

export function AccountNumberRow() {
  const pubkey = useSelector((state) => state.account.pubkey);
  return <Text variant="bodySmallSemibold" as={AccountNumberLabel} accountNumber={pubkey || ''} />;
}
