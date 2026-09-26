import styled from 'styled-components';

import { Accordion } from '../../../../../../../../../lib/components/accordion';
import {
  type AccordionHeaderProps,
  StyledAccordionContent,
  StyledAccordionHeader,
} from '../../../../../../../../../lib/components/accordion/components';
import { StyledAccordionHeaderItem } from '../../../../../../../../../lib/components/accordion/components/accordion-header/components';
import { StyledListItemTrailingAction } from '../../../../../../../../../lib/components/list-item/components/list-item-trailing-actions/components';
import {
  LocationAccordionHeaderItem,
  LocationAccordionTrailingActions,
  LocationAccordionTrigger,
} from './components';

export type LocationAccordionHeaderProps = AccordionHeaderProps;

export const LocationAccordionHeaderRoot = styled(Accordion.Header)``;

export const StyledLocationAccordionHeader = styled(LocationAccordionHeaderRoot)`
  // Remove the top border radius of all nested list items
  + ${StyledAccordionContent} {
    ${StyledAccordionHeader} {
      margin-top: 1px;
      ${StyledAccordionHeaderItem}, ${StyledListItemTrailingAction} {
        border-start-start-radius: 0;
        border-start-end-radius: 0;
      }
    }
  }

  // If followed by a list item
  &:has(~ ${LocationAccordionHeaderRoot}) {
    // Remove the bottom border radius of last list item
    + ${StyledAccordionContent} {
      ${StyledAccordionHeader}:last-child {
        ${StyledAccordionHeaderItem}, ${StyledListItemTrailingAction} {
          border-end-start-radius: 0;
          border-end-end-radius: 0;
        }
      }
    }
  }
`;

function LocationAccordionHeader({ children, ...props }: LocationAccordionHeaderProps) {
  return <StyledLocationAccordionHeader {...props}>{children}</StyledLocationAccordionHeader>;
}

const LocationAccordionHeaderNamespace = Object.assign(LocationAccordionHeader, {
  Item: LocationAccordionHeaderItem,
  TrailingActions: LocationAccordionTrailingActions,
  AccordionTrigger: LocationAccordionTrigger,
  ItemTrigger: Accordion.Header.ItemTrigger,
});

export { LocationAccordionHeaderNamespace as LocationAccordionHeader };
