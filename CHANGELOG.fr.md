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
