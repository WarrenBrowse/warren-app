import styled from 'styled-components';

import { measurements } from '../../../../common-styles';
import SearchBar, { type ISearchBarProps } from '../../../../SearchBar';

export type SearchBarProps = ISearchBarProps;

export const StyledSearchBar = styled(SearchBar)({
  marginInlineStart: measurements.horizontalViewMargin,
  marginInlineEnd: measurements.horizontalViewMargin,
  marginBottom: measurements.buttonVerticalMargin,
});

export function ApplicationSearchBar(props: SearchBarProps) {
  return <StyledSearchBar {...props} />;
}
