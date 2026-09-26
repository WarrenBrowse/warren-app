import { textDirection } from '../../shared/text-direction';

// Set on the document root rather than on the React tree so that portals (dialogs, menus, the
// country picker) inherit the direction too.
export function applyDocumentLocale(
  root: Pick<HTMLElement, 'lang' | 'dir'>,
  locale: string,
  hasCatalog: boolean,
) {
  const shown = hasCatalog ? locale : 'en';
  root.lang = shown;
  root.dir = textDirection(shown);
}

// Horizontal scroll offsets and arrow keys run the other way in a right-to-left element:
// `scrollLeft` goes from 0 towards negative values, and the next item is on the left.
export function inlineSign(element: Element): 1 | -1 {
  return getComputedStyle(element).direction === 'rtl' ? -1 : 1;
}
