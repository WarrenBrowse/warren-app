import styled from 'styled-components';

import { FontFamilies, surfaces } from '../../../foundations';
import { BodySmallSemiBoldProps, LabelTinySemiBold } from '../../text';
import { useFeatureIndicatorContext } from '../FeatureIndicatorContext';

export type FeatureIndicatorTextProps<T extends React.ElementType = 'span'> =
  BodySmallSemiBoldProps<T>;

export const StyledFeatureIndicatorText = styled(LabelTinySemiBold)<{ $disabled?: boolean }>`
  --color: ${({ $disabled }) => ($disabled ? surfaces.textMuted : surfaces.text)};
  font-family: ${FontFamilies.openSans};
  font-size: 11px;
  line-height: 15px;
  font-weight: 600;
  white-space: nowrap;
`;

export const FeatureIndicatorText = <T extends React.ElementType = 'span'>(
  props: FeatureIndicatorTextProps<T>,
) => {
  const { disabled } = useFeatureIndicatorContext();
  return <StyledFeatureIndicatorText $disabled={disabled} {...props} />;
};
