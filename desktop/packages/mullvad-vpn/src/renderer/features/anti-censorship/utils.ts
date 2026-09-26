import { isInRanges } from '../../../shared/utils';
import { joinList } from '../../lib/list-format';

export function validatePort(value: number, allowedPortRanges: [number, number][]): boolean {
  return isInRanges(value, allowedPortRanges);
}

export function validatePortString(value: string, allowedPortRanges: [number, number][]): boolean {
  const numericValue = parseInt(value, 10);
  if (Number.isNaN(numericValue)) return false;
  return validatePort(numericValue, allowedPortRanges);
}

export function formatPortRanges(portRanges: [number, number][]): string {
  return joinList(
    portRanges.map(([start, end]) => (start === end ? `${start}` : `${start}-${end}`)),
  );
}
