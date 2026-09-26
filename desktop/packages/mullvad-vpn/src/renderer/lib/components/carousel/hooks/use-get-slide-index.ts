import React from 'react';

import { inlineSign } from '../../../document-locale';
import { useCarouselContext } from '../CarouselContext';

// Calculate the slide index based on the scroll position.
export const useGetSlideIndex = () => {
  const { slides, slidesRef } = useCarouselContext();

  return React.useCallback(() => {
    if (slidesRef.current) {
      const scrollLeft = slidesRef.current.scrollLeft * inlineSign(slidesRef.current);
      const slideWidth = slidesRef.current.offsetWidth;

      // Clamp it between 0 and slides.length-1 to make sure it will correspond to a slide.
      return Math.max(0, Math.min(Math.round(scrollLeft / slideWidth), slides.length - 1));
    } else {
      return 0;
    }
  }, [slides.length, slidesRef]);
};
