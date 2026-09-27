import { messages } from '../../../../../../../../shared/gettext';
import {
  SectionTitle,
  type SectionTitleProps,
} from '../../../../../../../lib/components/section-title';

export type CustomListsSectionTitleProps = SectionTitleProps;

// No add button: a list is made from a place's lists menu, already holding it.
export function CustomListsSectionTitle({ id, ...props }: CustomListsSectionTitleProps) {
  return (
    <SectionTitle {...props}>
      <SectionTitle.Title as="h3" id={id}>
        {messages.pgettext('select-location-view', 'Custom lists')}
      </SectionTitle.Title>
      <SectionTitle.Divider />
    </SectionTitle>
  );
}
