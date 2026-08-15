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


## [1.1.16] - 2026-08-15
### Reparat
- [macOS] Nu mai lasă un computer fără rezolvare de nume după o deconectare. Când rula un alt VPN,
  aplicația putea înregistra adresa rezolverului privat al acelui VPN ca setare DNS originală a
  computerului, apoi o restabilea la deconectare. Acea adresă moare odată cu programul care o
  deținea, iar setarea în care era scrisă este o configurație de utilizator permanentă pe care
  nicio schimbare de rețea și nicio repornire nu o rescrie, așa că computerul rămânea îndreptat
  spre nimic până la o reconfigurare manuală a DNS-ului. Aplicația recunoaște acum o astfel de
  adresă ca fiind suprascrierea unui VPN, nu o mai înregistrează niciodată ca setare originală și
  o șterge pe cea rămasă, indiferent ce program a lăsat-o.
- [macOS] Nu mai întrerupe o conexiune care funcționează, la o secundă și jumătate după ce se
  stabilește. Aplicația verifică dacă traficul poate ieși din computer prin interfața de rețea pe
  care a ales-o, iar acea verificare număra pachete la care celălalt capăt nu poate răspunde
  niciodată, așa că declara moarte conexiuni sănătoase. Aplicația schimba apoi o rută și reconstruia
  un socket sub tunelul în funcțiune ca să le repare, iar acea schimbare este cea care ucidea
  traficul. Tunelul rămânea conectat fără să transporte nimic și se reconecta la fiecare 15 secunde.
  Verificarea numără acum doar ceea ce poate primi un răspuns și se uită la toate conexiunile
  tunelului, nu mai atinge niciodată un tunel în funcțiune, iar un computer care chiar are nevoie de
  ruta de rezervă o primește înainte de pornirea tunelului, acolo unde funcționează.
- Nu mai propune asistentul de creare a contului cuiva care tocmai a restaurat un cont existent din
  fraza sa de recuperare. Acel cont are deja portofelul și abonamentul, deci asistentul nu are ce să
  configureze. Valabil pentru aplicația de desktop și pentru iOS.
- Nu mai aduce înapoi asistentul după ce a fost terminat sau sărit. La câteva minute după ce
  fereastra este ascunsă, aplicația revine la vizualizarea de pornire, iar acea vizualizare rămânea
  asistentul, care reapărea la fiecare redeschidere.


## [1.1.14] - 2026-08-14
### Reparat
- [macOS] Instalează aplicația pe un computer care nu a avut-o niciodată. Începând cu 1.1.6,
  programul de instalare refuza orice primă instalare cu singurul mesaj „a apărut o eroare la
  rularea scripturilor pachetului”: un pas care protejează actualizările căuta versiunea anterioară
  a aplicației și oprea instalarea când nu o găsea. Actualizarea unei instalări existente nu a fost
  niciodată afectată.
- [macOS] Restabilește accesul la internet când conexiunea nu poate ieși din computer prin
  interfața de rețea aleasă de aplicație. Aplicația reținea că o rută de rezervă fusese pusă fără
  să verifice vreodată că funcționează și păstra acest verdict o săptămână: un computer a cărui
  rută de rezervă era și ea moartă rămânea „Conectat” fără să ajungă nicăieri, reconectându-se la
  fiecare 15 secunde.
- Nu mai întrerupe o conexiune sănătoasă care încă transportă trafic. Verificarea care urmărește un
  server mort își număra eșecurile pe câteva secunde fără să se uite la datele care încă soseau,
  așa că o conexiune la internet saturată era de ajuns pentru a distruge un tunel funcțional și a
  omorî toate cererile în curs.
- Spune ce a eșuat cu adevărat când trimiterea jurnalelor către forumul de asistență nu reușește.
  Toate erorile afișau același mesaj „încercați din nou”, inclusiv cele pe care o nouă încercare nu
  le poate rezolva.


## [1.1.11] - 2026-08-09
### Adăugat
- Adaugă comanda `warren-beta` și pe Windows. macOS și Linux instalează deja linia de comandă sub
  numele propriu mediului, iar Windows cunoștea doar `warren`, așa că niciun nume de comandă nu
  funcționa pe toate sistemele. `warren` continuă să funcționeze pe Windows.

### Reparat
- Limitează în timp verificarea de versiune care precedă fiecare actualizare, astfel încât ecranul
  de actualizare să nu mai poată rămâne blocat la nesfârșit pe „Se începe descărcarea...” la 0%.
  O verificare a cărei conexiune îngheța nu răspundea niciodată: descărcarea pe care o
  condiționează, orice cerere de actualizare și controlul periodic așteptau până la repornirea
  aplicației. Acum renunță după cel mult un minut, iar actualizarea continuă cu ultimele informații
  de versiune cunoscute.
- Reflectă punerea în pauză a descărcării actualizării în ecranul de actualizare chiar și atunci
  când descărcarea nu a început încă. O pauză la 0% anula descărcarea fără să anunțe ecranul, care
  continua să afișeze „Se începe descărcarea...” până la repornirea aplicației.


## [1.1.10] - 2026-08-09
### Reparat
- Elimină la pornire regulile de firewall Warren lăsate pe mașină de un mediu de produs care nu mai
  este instalat. Versiunea 1.1.9 pentru Windows fusese construită cu identificatori interni de
  firewall greșiți: regulile de blocare lăsate de versiunea precedentă în timpul actualizării îi
  erau invizibile, iar calculatorul rămânea fără internet, chiar și cu VPN oprit, până la o
  reparație manuală. Mașinile care se actualizează de la 1.1.9 se repară singure la prima pornire a
  noii versiuni.
- Verifică octet cu octet componenta de firewall a fiecărei versiuni Windows înainte de publicare,
  astfel încât o versiune cu identificatorii altui mediu să nu mai poată fi publicată niciodată.


## [1.1.9] - 2026-08-08
### Adăugat
- Un acces către forumul comunității din antetul aplicației, chiar înainte de a avea un cont pe
  forum. Locul care va găzdui clopoțelul de activitate arată un colac de salvare care deschide
  forumul. După autentificarea acolo, devine clopoțelul. Oprirea notificărilor de forum le
  elimină pe amândouă.
- Publicarea Warren VPN pentru Linux ARM pe 64 de biți. Pachetele `.deb`, `.rpm` și Arch există
  acum și în varianta arm64 alături de x86_64, astfel încât un Raspberry Pi 4 sau 5, un laptop
  ARM sau o instanță cloud ARM instalează aplicația direct în loc să nu aibă nimic de descărcat.
- Publicarea daemonului fără interfață și a liniei de comandă `warren` și pentru Linux ARM pe 64
  de biți, ca scriptul de instalare pentru server să funcționeze pe o mașină ARM.
- Se spune când o cerere de autentificare pe forum vine dintr-un cod QR: browserul autentificat
  este pe alt dispozitiv, iar mesajul precizează să aprobați doar dacă dumneavoastră ați pornit
  acea autentificare.

### Modificat
- O actualizare pornește de la o listă de versiuni adusă chiar în acel moment, nu de la ultima
  verificare. Aplicația reverifică doar la fiecare șase ore, deci o versiune publicată între timp
  era invizibilă: ecranul de actualizare putea propune, descărca și instala o versiune pe care
  canalul o înlocuise deja.

### Reparat
- Nu se mai blochează pe „Pornirea programului de instalare...”. Când o versiune apărea între
  descărcare și pornirea programului de instalare, aplicația abandona instalatorul verificat fără
  să anunțe pe nimeni, iar ecranul de actualizare aștepta la nesfârșit; acum reia singură
  actualizarea către versiunea mai nouă. O pornire eșuată a programului de instalare este acum
  anunțată în loc să nu spună nimic.
- O verificare de versiune care eșuează (offline, gazdă de actualizare inaccesibilă) răspunde cu
  ultimele versiuni cunoscute, în loc să lase ecranul de actualizare să aștepte până la următoarea
  verificare reușită.
- „Descărcare finalizată!” și celelalte texte de progres ale descărcării se afișează în limba
  dumneavoastră. Erau construite înainte de încărcarea traducerilor și rămâneau în engleză
  indiferent de limba aleasă.
- Notele de versiune în franceză și română pentru 1.1.6 până la 1.1.8, afișate în engleză:
  jurnalele traduse se opriseră în tăcere la 1.1.4. O versiune nouă refuză acum să publice note
  netraduse în fiecare limbă livrată, iar intrările deja publicate doar în engleză sunt reparate
  la următoarea publicare.
- Jurnalele aplicației se pot atașa din nou la un raport de eroare de pe forum când raportul este
  mare. Aplicația refuza să semneze orice raport de peste 1 MiB comprimat, în timp ce toate
  celelalte limite din lanț permiteau 12, așa că un raport îngrășat de câteva zile de jurnale
  eșua cu o eroare generică „încercați din nou”.


## [1.1.8] - 2026-08-08
### Adăugat
- Marcarea tuturor notificărilor de forum ca citite, dintr-un buton aflat în partea de sus a
  panoului forumului. Le marchează citite și pe forum, așa că propriul lui contor urmează.

### Modificat
- Închiderea panoului forumului și revenirea la ecranul principal la deschiderea unei notificări,
  în loc să rămână lista în spatele browserului.
- Excluderea insignelor din panoul forumului și din contorul lui. Erau cea mai frecventă
  notificare și singura pe care nu v-o trimite nimeni.
- Actualizarea contorului forumului în momentul în care acționați, nu la următoarea verificare.
  Deschiderea panoului, deschiderea unei notificări sau marcarea tuturor ca citite mută clopoțelul
  și punctul imediat.
- Verificarea activității de pe forum în fiecare minut în loc de cinci, astfel încât ceva citit în
  altă parte dispare de aici mai repede.

### Reparat
- Numărarea notificărilor necitite exact ca forumul, inclusiv excluderea celor al căror subiect a
  fost șters între timp.


## [1.1.7] - 2026-08-08
### Adăugat
- Vă anunță când vi se întâmplă ceva nou pe forumul comunității, printr-o notificare de birou și
  același punct pe care aplicația îl pune deja pe pictograma ei din bara de sistem pentru o
  actualizare. Ambele se sting singure după ce ați citit forumul, oriunde l-ați citit.
- O setare „Notificări de forum”, în setările interfeței, pentru a le opri. Este pornită implicit
  și apare doar după ce aveți un cont pe forum.

### Modificat
- Redesenarea panoului de activitate al forumului. Fiecare notificare este acum propriul ei card,
  cu o pictogramă care deosebește un răspuns de o apreciere dintr-o privire, subiectul, un extras
  din ce s-a scris și cât timp a trecut. Textele lungi nu mai ies din fereastră.

### Reparat
- Afișarea aceluiași număr de notificări necitite ca forumul însuși. Aplicația număra fiecare
  notificare nedeschisă individual, în timp ce forumul nu le mai numără după ce lista a fost
  consultată, așa că aplicația putea anunța trei când nu era niciuna.
- Deschiderea unei notificări de forum direct la mesaj. Un clic rula înainte întreaga
  autentificare prin brokerul de identitate, chiar și când browserul era deja autentificat.
- Vechimea unei notificări nu se mai afișează ca durată negativă. În franceză se afișa „-2 h”
  pentru acum două ore.
- Fiecare tip de notificare de forum spune ce s-a întâmplat de fapt. Reacțiile, subiectele noi
  dintr-o categorie urmărită și rezumatele mesajelor de grup afișau toate „Activitate nouă pe
  forum”.
- Traducerea aplicației în toate limbile livrate. Panoul forumului, notificările lui, linkul
  condițiilor de utilizare și avertismentele redirecționării de porturi apăreau în engleză
  indiferent de limba aleasă.


## [1.1.6] - 2026-08-08
### Adăugat
- Un clopoțel de activitate a forumului în aplicație, cu un panou care listează ce vi se întâmplă
  pe forumul comunității. Contorul vine dintr-un document identic pentru toți clienții, deci
  serverului nu i se cere nimic despre contul dumneavoastră pentru a desena insigna.

### Reparat
- Fereastra aplicației se deschide din nou. 1.1.5 livra o interfață care se bloca la încărcare,
  fereastra rămânea goală și clicul pe pictogramă nu făcea nimic. Acea versiune a fost retrasă.
- Tunelul nu se mai reconstruiește când rețeaua îngheață câteva secunde. Un blocaj scurt (o
  schimbare de Wi-Fi, o legătură saturată, un semnal slab) era de ajuns ca aplicația să decidă că
  serverul nu mai retransmite și să reconstruiască tunelul, ceea ce tăia toate cererile în curs.
  Acum verifică dacă mai ajunge ceva la server înainte de a-l acuza. Un server care chiar a
  încetat să retransmită este detectat la fel de repede ca înainte.
- Calculatorul nu mai rămâne fără internet după o actualizare a aplicației. Actualizarea
  sigilează rețeaua cât timp aplicația este înlocuită, ceea ce este intenționat, dar nimic nu
  ridica acest sigiliu dacă actualizarea eșua pe drum. O protecție eliberează acum mașina singură,
  iar aplicația nu mai așteaptă o jumătate de minut după o căutare de nume pe care propria ei
  protecție o bloca.
- Reconectarea de la sine după o actualizare, în loc de a rămâne blocată până la un clic.


## [1.1.4] - 2026-08-08
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
