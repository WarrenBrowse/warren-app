import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import { LocationFlag } from '../../src/renderer/components/views/main/home/LocationFlag';

// The location row pictures the country with its round flag, as the mockup does.
describe('LocationFlag', () => {
  it('shows the round flag of the country it is given', () => {
    const html = renderToStaticMarkup(<LocationFlag code="FI" />);
    expect(html).toContain('src="assets/images/flags/fi.svg"');
  });

  it('shows nothing for a pick that names no country', () => {
    expect(renderToStaticMarkup(<LocationFlag code={undefined} />)).toBe('');
  });
});
