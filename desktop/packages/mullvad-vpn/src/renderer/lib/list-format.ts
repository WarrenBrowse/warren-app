import { messages } from '../../shared/gettext';

// Joins the items of a short enumeration (forwarded ports, port ranges, a location's parents)
// with the separator of the active language, e.g. the Arabic comma in Arabic and Persian.
export function joinList(items: string[]): string {
  return items.join(
    // TRANSLATORS: Separator placed between the items of a short list, such as forwarded ports
    // TRANSLATORS: ("6881 TCP, 51413 UDP") or a city and its country. Keep the trailing space if
    // TRANSLATORS: your language puts one after its comma.
    messages.pgettext('list-format', ', '),
  );
}
