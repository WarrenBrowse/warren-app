import React from 'react';

import { inlineSign } from '../../../document-locale';
import { useSlides } from './use-slides';

export const useHandleKeyboardNavigation = () => {
  const { goToNextSlide, goToPreviousSlide } = useSlides();
  return React.useCallback(
    (event: React.KeyboardEvent) => {
      if (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight') {
        return;
      }
      event.preventDefault();
      const towardsEnd = (event.key === 'ArrowRight') === (inlineSign(event.currentTarget) === 1);
      if (towardsEnd) {
        goToNextSlide();
      } else {
        goToPreviousSlide();
      }
    },
    [goToNextSlide, goToPreviousSlide],
  );
};
