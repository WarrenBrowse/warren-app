import { messages } from '../../../../../../shared/gettext';
import { useActiveFilters } from '../../../../../features/locations/hooks/use-active-filters';
import { Flex, LabelTinySemiBold } from '../../../../../lib/components';
import { useSelectLocationViewContext } from '../../SelectLocationViewContext';
import { DaitaFilterChip } from '../daita-filter-chip';
import { OwnershipFilterChip } from '../ownership-filter-chip';
import { ProvidersFilterChip } from '../providers-filter-chip';

export function FilterChips() {
  const { locationType } = useSelectLocationViewContext();
  const { isOwnershipFilterActive, isProvidersFilterActive, isDaitaFilterActive } =
    useActiveFilters(locationType);

  return (
    <Flex
      gap="small"
      alignItems="center"
      flexWrap="wrap"
      margin={{ horizontal: 'small', bottom: 'medium' }}>
      <LabelTinySemiBold>
        {messages.pgettext('select-location-view', 'Filtered:')}
      </LabelTinySemiBold>

      {isOwnershipFilterActive && <OwnershipFilterChip />}
      {isProvidersFilterActive && <ProvidersFilterChip />}
      {isDaitaFilterActive && <DaitaFilterChip />}
    </Flex>
  );
}
