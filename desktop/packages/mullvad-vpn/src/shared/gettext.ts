import { GetTextTranslations } from 'gettext-parser';
import Gettext from 'node-gettext';

import { LocalizationContexts } from './localization-contexts';
import log from './logging';

const SOURCE_LANGUAGE = 'en';

function setErrorHandler(catalogue: Gettext) {
  catalogue.on('error', (error) => {
    log.debug(`Gettext error: ${error.message}`);
  });
}

const gettextOptions = { sourceLocale: SOURCE_LANGUAGE };

declare class GettextWithAppContexts extends Gettext {
  pgettext(msgctxt: LocalizationContexts, msgid: string): string;
  npgettext(
    msgctxt: LocalizationContexts,
    msgid: string,
    msgidPlural: string,
    count: number,
  ): string;
}

// node-gettext stores catalogues at catalogs[locale][domain] on plain objects,
// so one of these keys as a locale or domain writes onto Object.prototype
// (CVE-2024-21528, still present in node-gettext 3.0.1 although no scanner
// reports that version).
const RESERVED_KEYS = new Set(['__proto__', 'constructor', 'prototype']);

export function addTranslations(
  catalogue: Gettext,
  locale: string,
  domain: string,
  translations: GetTextTranslations | object,
): boolean {
  if (RESERVED_KEYS.has(locale) || RESERVED_KEYS.has(domain)) {
    log.error('Refused a translation catalogue with a reserved locale or domain name');
    return false;
  }
  catalogue.addTranslations(locale, domain, translations);
  return true;
}

export const messages = new Gettext(gettextOptions) as GettextWithAppContexts;
messages.setTextDomain('messages');
setErrorHandler(messages);

export const relayLocations = new Gettext(gettextOptions);
relayLocations.setTextDomain('relay-locations');
setErrorHandler(relayLocations);
