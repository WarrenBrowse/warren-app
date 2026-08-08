# Journal des modifications

Traduction française des notes de version affichées dans l'application au
moment de proposer une mise à jour. `ci/build-version-metadata.py` extrait la
section de la version publiée et la place dans le manifeste signé, à côté de la
version anglaise.

Une version absente de ce fichier s'affiche en anglais, ce qui est le
comportement voulu : mieux vaut des notes dans une autre langue que pas de
notes du tout. Les en-têtes `## [X.Y.Z]` doivent rester identiques à ceux de
`CHANGELOG.md`, l'extraction se fait par correspondance sur le numéro de
version. Le préfixe de plateforme (`[macOS]`, `[Windows]`, `[linux]`) est lu
par l'application, gardez-le tel quel.

## [Non publié]


## [1.1.4] - 2026-08-08
### Corrigé
- [macOS] L'installation est possible sur les Mac Intel. Le paquet macOS était compilé pour Apple
  Silicon uniquement tout en étant publié comme universel, si bien que le programme d'installation
  le refusait sur tout Mac Intel avec « Warren VPN Beta ne peut pas être installé sur cet
  ordinateur », quelle que soit la version de macOS.


## [1.1.2] - 2026-08-05
### Corrigé
- [macOS] L'application joint à nouveau l'API Warren pendant que le tunnel est actif. Depuis la
  1.1.0, elle ne pouvait plus vérifier les mises à jour, rafraîchir votre compte ni renouveler ses
  jetons d'accès une fois connectée, et affichait un abonnement actif comme inactif.

## [1.1.1] - 2026-08-05
### Modifié
- Affiner l'illustration de l'écran de connexion : nouvelle pose de Bula, cadrage décentré et
  ligne de sol retouchée.

## [1.1.0] - 2026-08-05
### Ajouté
- Afficher les notes de version d'une mise à jour dans la langue de
  l'application, lorsque la version publiée en propose une traduction.
  L'anglais reste la solution de repli.

### Corrigé
- [macOS] Les sites ne se figent plus pendant les premières minutes suivant une
  connexion ou un changement de serveur. Le tunnel ne transporte pas d'IPv6
  sauf si vous l'activez, mais le Mac conservait une adresse IPv6 globale
  fonctionnelle : il tentait donc l'IPv6 en premier pour chaque site à double
  pile, et seul le pare-feu l'arrêtait, tardivement et de façon peu fiable.
  L'IPv6 est désormais déclarée injoignable tant que le tunnel n'en transporte
  pas, et ces sites passent directement en IPv4.
- Les notes de version d'une mise à jour s'affichent avec leur mise en forme.
  Les titres, les listes et les emphases apparaissaient en Markdown brut, et
  une entrée répartie sur plusieurs lignes était découpée en plusieurs puces.
