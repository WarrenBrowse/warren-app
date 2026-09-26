import styled, { css } from 'styled-components';

import { Spacings, spacings } from '../../foundations';
import { Flex, FlexProps } from '../flex';

type HorizontalMargin = Extract<Spacings, 'small' | 'medium' | 'large'>;

export type ContainerProps = FlexProps & {
  horizontalMargin: HorizontalMargin;
};

export const StyledContainer = styled(Flex)<{ $horizontalMargin: string }>`
  ${({ $horizontalMargin }) => css`
    margin-inline-start: ${$horizontalMargin};
    margin-inline-end: ${$horizontalMargin};
  `}
`;

export function Container({ horizontalMargin, ...props }: ContainerProps) {
  const spacing = spacings[horizontalMargin];
  return <StyledContainer $horizontalMargin={spacing} {...props} />;
}
