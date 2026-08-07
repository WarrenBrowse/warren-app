# Jurnal de modificări

Traducerea în română a notelor de versiune afișate în aplicație atunci când
este propusă o actualizare. `ci/build-version-metadata.py` extrage secțiunea
versiunii publicate și o adaugă în manifestul semnat, alături de varianta în
engleză.

O versiune care lipsește din acest fișier se afișează în engleză, iar acesta
este comportamentul dorit: mai bine note într-o altă limbă decât nicio notă.
Titlurile `## [X.Y.Z]` trebuie să rămână identice cu cele din `CHANGELOG.md`,
extragerea se face după numărul de versiune. Prefixul de platformă (`[macOS]`,
`[Windows]`, `[linux]`) este citit de aplicație, păstrați-l ca atare.

## [Nepublicat]


## [1.1.3] - 2026-08-08
### Reparat
- [macOS] Instalarea este posibilă pe Mac-urile Intel. Pachetul macOS era compilat doar pentru
  Apple Silicon deși era publicat ca universal, așa că programul de instalare îl refuza pe orice
  Mac Intel cu „Warren VPN Beta nu poate fi instalat pe acest computer”, indiferent de versiunea
  de macOS.


## [1.1.2] - 2026-08-05
### Reparat
- [macOS] Aplicația contactează din nou API-ul Warren cât timp tunelul este activ. Începând cu
  1.1.0 nu mai putea verifica actualizările, reîmprospăta contul sau reînnoi jetoanele de acces
  odată conectată, iar un abonament activ apărea ca inactiv.

## [1.1.1] - 2026-08-05
### Modificat
- Rafinarea ilustrației din ecranul de conectare: o nouă poziție a lui Bula, încadrare
  descentrată și o linie a solului ajustată.

## [1.1.0] - 2026-08-05
### Adăugat
- Afișarea notelor de versiune ale unei actualizări în limba aplicației, atunci
  când versiunea publicată include o traducere. Engleza rămâne varianta
  implicită.

### Reparat
- [macOS] Site-urile nu mai rămân blocate în primele minute după conectare sau
  după schimbarea serverului. Tunelul nu transportă IPv6 decât dacă îl
  activați, însă Mac-ul păstra o adresă IPv6 globală funcțională: încerca deci
  IPv6 mai întâi pentru fiecare site dual-stack, iar numai firewallul îl
  oprea, târziu și în mod nesigur. IPv6 este acum declarat inaccesibil cât timp
  tunelul nu îl transportă, iar aceste site-uri trec direct pe IPv4.
- Notele de versiune ale unei actualizări sunt afișate formatat. Titlurile,
  listele și accentuările apăreau ca Markdown brut, iar o intrare împărțită pe
  mai multe linii era ruptă în mai multe puncte.
