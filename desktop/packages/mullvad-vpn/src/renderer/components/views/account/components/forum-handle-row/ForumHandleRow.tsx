import { Text } from '../../../../../lib/components';
import { useSelector } from '../../../../../redux/store';

// The wallet's community-forum name. Rendered only once known: it is derived
// server side with a keyed HMAC, so the app learns it from the forum sign-in
// response and has nothing to show before the first one.
export function ForumHandleRow() {
  const forumHandle = useSelector((state) => state.account.forumHandle);
  return <Text variant="bodySmallSemibold">{forumHandle}</Text>;
}
