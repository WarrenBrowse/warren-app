import { expect, it } from 'vitest';

import { formatErrorDetails } from '../../src/renderer/lib/error-details';

it('leads with the error name and message', () => {
  const details = formatErrorDetails(new RangeError('index out of bounds'));
  expect(details.startsWith('RangeError: index out of bounds')).toBe(true);
});

it('appends trimmed component stack frames', () => {
  const details = formatErrorDetails(new Error('boom'), '\n    in Foo\n    in Bar\n');
  expect(details).toBe('Error: boom\nin Foo\nin Bar');
});

it('caps a deep component stack to keep the view readable', () => {
  const stack = Array.from({ length: 50 }, (_, i) => `    in Component${i}`).join('\n');
  const details = formatErrorDetails(new Error('boom'), stack);
  const lines = details.split('\n');
  expect(lines.length).toBeLessThanOrEqual(13);
  expect(lines[lines.length - 1]).toBe('...');
});

it('works without a component stack', () => {
  expect(formatErrorDetails(new Error('boom'))).toBe('Error: boom');
});
