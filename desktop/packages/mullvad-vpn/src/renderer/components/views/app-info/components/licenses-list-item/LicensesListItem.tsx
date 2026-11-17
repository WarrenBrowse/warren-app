import { useCallback } from 'react';

import { RoutePath } from '../../../../../../shared/routes';
import { Icon } from '../../../../../lib/components';
import { ListItem } from '../../../../../lib/components/list-item';
import { useHistory } from '../../../../../lib/history';
import { licensesTitle } from '../../../licenses';

export function LicensesListItem() {
  const history = useHistory();
  const pushLicenses = useCallback(() => history.push(RoutePath.licenses), [history]);

  return (
    <ListItem>
      <ListItem.Trigger onClick={pushLicenses}>
        <ListItem.Item>
          <ListItem.Item.Label>{licensesTitle()}</ListItem.Item.Label>
          <ListItem.Item.ActionGroup>
            <Icon icon="chevron-right" />
          </ListItem.Item.ActionGroup>
        </ListItem.Item>
      </ListItem.Trigger>
    </ListItem>
  );
}
