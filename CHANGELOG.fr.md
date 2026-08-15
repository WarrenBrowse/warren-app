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


## [1.1.16] - 2026-08-15
### Corrigé
- [macOS] Ne plus couper une connexion qui fonctionne, une seconde et demie après son
  établissement. L'application vérifie que le trafic peut sortir de l'ordinateur par l'interface
  réseau qu'elle a choisie, et ce contrôle comptait des paquets auxquels l'autre bout ne peut jamais
  répondre, si bien qu'il déclarait mortes des connexions saines. L'application remplaçait alors une
  route et recréait une socket sous le tunnel en cours pour les réparer, et c'est ce remplacement
  qui tuait le trafic. Le tunnel restait connecté sans rien transporter et se reconnectait toutes
  les 15 secondes. Le contrôle ne compte plus que ce à quoi l'autre bout peut répondre et regarde
  toutes les connexions du tunnel, il ne touche plus jamais à un tunnel en cours, et un ordinateur
  qui a réellement besoin de la route de secours la reçoit avant le démarrage du tunnel, là où elle
  fonctionne.
- Ne plus proposer l'assistant de création de compte à quelqu'un qui vient de restaurer un compte
  existant depuis sa phrase de récupération. Ce compte porte déjà son portefeuille et son
  abonnement, l'assistant n'a donc rien à configurer. Vaut pour l'application de bureau et pour iOS.
- Ne plus faire revenir l'assistant une fois qu'il a été terminé ou passé. Quelques minutes après
  la fermeture de la fenêtre, l'application revient à sa vue de départ, et cette vue de départ
  restait l'assistant, qui réapparaissait donc à chaque réouverture.


## [1.1.14] - 2026-08-14
### Corrigé
- [macOS] Installer l'application sur un ordinateur qui ne l'a jamais eue. Depuis la 1.1.6,
  l'installateur refusait toute première installation avec pour seul message « une erreur s'est
  produite pendant l'exécution des scripts du paquet » : une étape qui protège les mises à jour
  cherchait la version précédente de l'application et interrompait l'installation faute de la
  trouver. La mise à jour d'une installation existante n'a jamais été touchée.
- [macOS] Rétablir l'accès à internet quand la connexion ne peut pas sortir de l'ordinateur par
  l'interface réseau choisie par l'application. L'application retenait qu'une route de secours
  avait été posée sans jamais vérifier qu'elle fonctionnait, et gardait ce verdict une semaine :
  un ordinateur dont la route de secours était morte elle aussi restait « Connecté » sans rien
  joindre, en se reconnectant toutes les 15 secondes.
- Ne plus couper une connexion saine qui transporte encore du trafic. Le contrôle qui surveille un
  serveur mort comptait ses échecs sur quelques secondes sans regarder les données qui arrivaient
  encore : une connexion internet saturée suffisait à détruire un tunnel qui marchait et à tuer
  toutes les requêtes en cours.
- Dire ce qui a réellement échoué quand l'envoi des journaux au forum d'entraide ne passe pas.
  Toutes les erreurs affichaient le même message « réessayez », y compris celles qu'un nouvel essai
  ne peut pas corriger.


## [1.1.11] - 2026-08-09
### Ajouté
- Ajouter la commande `warren-beta` sur Windows aussi. macOS et Linux installent déjà la ligne de
  commande sous le nom propre à l'environnement, et Windows ne connaissait que `warren` : aucun nom
  de commande unique ne marchait sur tous les systèmes. `warren` continue de fonctionner sur
  Windows.

### Corrigé
- Borner dans le temps la vérification de version qui précède chaque mise à jour, pour que l'écran
  de mise à jour ne puisse plus rester indéfiniment sur « Démarrage du téléchargement… » à 0 %.
  Une vérification dont la connexion se figeait ne répondait jamais : le téléchargement qu'elle
  conditionne, toute demande de mise à jour et le contrôle périodique attendaient jusqu'au
  redémarrage de l'application. Elle abandonne désormais au bout d'une minute au plus et la mise à
  jour continue avec les dernières informations de version connues.
- Refléter la mise en pause du téléchargement de mise à jour dans l'écran de mise à jour même quand
  le téléchargement n'a pas encore commencé. Une pause à 0 % annulait le téléchargement sans en
  informer l'écran, qui continuait d'afficher « Démarrage du téléchargement… » jusqu'au redémarrage
  de l'application.


## [1.1.10] - 2026-08-09
### Corrigé
- Retirer au démarrage les règles de pare-feu Warren laissées sur la machine par un environnement
  produit qui n'est plus installé. La version Windows 1.1.9 avait été fabriquée avec de mauvais
  identifiants internes de pare-feu : les règles de blocage posées par la version précédente
  pendant la mise à jour lui étaient invisibles et l'ordinateur restait coupé d'internet, même VPN
  éteint, jusqu'à une réparation manuelle. Les machines qui se mettent à jour depuis la 1.1.9 se
  réparent seules au premier démarrage de la nouvelle version.
- Vérifier octet par octet le composant pare-feu de chaque version Windows avant publication, pour
  qu'une version portant les identifiants d'un autre environnement ne puisse plus jamais paraître.


## [1.1.9] - 2026-08-08
### Ajouté
- Offrir un accès au forum communautaire depuis l'en-tête de l'application avant même d'avoir un
  compte forum. L'emplacement qui accueillera la cloche d'activité affiche une bouée qui ouvre le
  forum. Une fois connecté là-bas, elle devient la cloche. Couper les notifications du forum
  retire les deux.
- Publier Warren VPN pour Linux ARM 64 bits. Les paquets `.deb`, `.rpm` et Arch existent
  désormais en arm64 à côté du x86_64, si bien qu'un Raspberry Pi 4 ou 5, un portable ARM ou une
  instance cloud ARM installe l'application directement au lieu de n'avoir rien à télécharger.
- Publier aussi le démon sans interface et la ligne de commande `warren` pour Linux ARM 64 bits,
  pour que le script d'installation serveur fonctionne sur une machine ARM.
- Le dire quand une demande de connexion au forum vient d'un code QR : le navigateur connecté est
  sur un autre appareil, et l'invite précise de n'approuver que si vous êtes à l'origine de cette
  connexion.

### Modifié
- Démarrer une mise à jour depuis une liste de versions récupérée à l'instant même plutôt que
  depuis la dernière vérification. L'application ne revérifie que toutes les six heures, donc une
  version publiée entre-temps était invisible : l'écran de mise à jour pouvait proposer,
  télécharger et installer une version que le canal avait déjà remplacée.

### Corrigé
- Ne plus rester bloqué sur « Démarrage de l'installateur… ». Quand une version sortait entre le
  téléchargement et le lancement de l'installateur, l'application abandonnait l'installateur
  vérifié sans prévenir personne et l'écran de mise à jour attendait pour toujours ; elle relance
  désormais d'elle-même la mise à niveau vers la version plus récente. Un échec du lancement de
  l'installateur le dit maintenant au lieu de ne rien dire.
- Répondre à une vérification de version qui échoue (hors ligne, hôte de mise à jour
  injoignable) avec les dernières versions connues, au lieu de laisser l'écran de mise à jour
  attendre la prochaine vérification réussie.
- Afficher « Téléchargement terminé ! » et les autres textes de progression du téléchargement
  dans votre langue. Ils étaient construits avant le chargement des traductions et restaient en
  anglais quelle que soit la langue choisie.
- Servir les notes de version françaises et roumaines des 1.1.6 à 1.1.8, affichées en anglais :
  les journaux traduits s'étaient arrêtés en silence à la 1.1.4. Une release refuse désormais de
  publier des notes non traduites dans chaque langue livrée, et les entrées déjà publiées en
  anglais seul sont réparées à la prochaine publication.
- Joindre à nouveau les journaux de l'application à un rapport de bug du forum quand le rapport
  est volumineux. L'application refusait de signer tout rapport dépassant 1 Mio compressé alors
  que toutes les autres limites de la chaîne en autorisaient 12, si bien qu'un rapport gonflé par
  quelques jours de journaux échouait avec une erreur générique « réessayez ».


## [1.1.8] - 2026-08-08
### Ajouté
- Marquer toutes les notifications du forum comme lues, depuis un bouton en haut du panneau du
  forum. Elles sont aussi marquées lues sur le forum, dont le compteur suit.

### Modifié
- Fermer le panneau du forum et revenir à la vue principale à l'ouverture d'une notification, au
  lieu de laisser la liste derrière le navigateur.
- Écarter les badges du panneau du forum et de son compteur. C'était la notification la plus
  fréquente et la seule que personne ne vous envoie.
- Mettre à jour le compteur du forum dès que vous agissez, au lieu d'attendre la prochaine
  vérification. Ouvrir le panneau, ouvrir une notification ou tout marquer lu déplace la cloche et
  le point d'un coup.
- Vérifier l'activité du forum chaque minute au lieu de toutes les cinq minutes, pour qu'une
  lecture faite ailleurs disparaisse ici plus vite.

### Corrigé
- Compter les notifications non lues exactement comme le forum, y compris en écartant celles dont
  le sujet a été supprimé depuis.


## [1.1.7] - 2026-08-08
### Ajouté
- Vous prévenir quand quelque chose de nouveau vous arrive sur le forum communautaire, avec une
  notification de bureau et le même point que l'application met déjà sur son icône de la barre
  système pour une mise à jour. Les deux s'éteignent d'eux-mêmes une fois le forum lu, où que ce
  soit.
- Ajouter un réglage « Notifications du forum », dans les réglages d'interface, pour les couper.
  Il est actif par défaut et n'apparaît qu'une fois votre compte forum créé.

### Modifié
- Redessiner le panneau d'activité du forum. Chaque notification est désormais sa propre carte,
  avec une icône qui distingue une réponse d'un « j'aime » au premier regard, le sujet, un extrait
  de ce qui a été écrit et l'ancienneté. Les textes longs ne débordent plus de la fenêtre.

### Corrigé
- Afficher le même nombre de notifications non lues que le forum lui-même. L'application comptait
  chaque notification pas encore ouverte individuellement, alors que le forum cesse de les compter
  dès que la liste a été consultée, si bien que l'application pouvait en annoncer trois quand il
  n'y en avait aucune.
- Ouvrir une notification du forum directement sur le message. Un clic déroulait auparavant toute
  la connexion via le courtier d'identité, même quand le navigateur était déjà connecté.
- Ne plus afficher l'ancienneté d'une notification en durée négative. En français, « -2 h »
  s'affichait pour il y a deux heures.
- Dire ce qui s'est réellement passé pour chaque type de notification du forum. Les réactions, les
  nouveaux sujets d'une catégorie suivie et les résumés de messages de groupe affichaient tous
  « Nouvelle activité sur le forum ».
- Traduire l'application dans toutes les langues livrées. Le panneau du forum, ses notifications,
  le lien des conditions d'utilisation et les avertissements de redirection de ports s'affichaient
  en anglais quelle que soit la langue choisie.


## [1.1.6] - 2026-08-08
### Ajouté
- Afficher une cloche d'activité du forum dans l'application, avec un panneau listant ce qui vous
  arrive sur le forum communautaire. Le compteur vient d'un document identique pour tous les
  clients, donc le serveur n'apprend rien de votre compte pour dessiner le badge.

### Corrigé
- Rouvrir la fenêtre de l'application. La 1.1.5 livrait une interface qui plantait au chargement,
  la fenêtre restait vide et le clic sur l'icône ne faisait rien. Cette version a été retirée.
- Ne plus reconstruire le tunnel quand le réseau cale quelques secondes. Un bref blocage (un
  changement de Wi-Fi, une liaison saturée, un signal faible) suffisait pour que l'application
  décide que le serveur ne relayait plus et reconstruise le tunnel, ce qui coupait toutes les
  requêtes en cours. Elle vérifie désormais si quelque chose atteint encore le serveur avant de
  l'accuser. Un serveur qui a réellement cessé de relayer est détecté aussi vite qu'avant.
- Ne plus laisser l'ordinateur sans internet après une mise à jour de l'application. La mise à
  jour scelle le réseau pendant le remplacement de l'application, ce qui est voulu, mais rien ne
  levait ce scellé si la mise à jour échouait en route. Un garde-fou libère maintenant la machine
  de lui-même, et l'application n'attend plus trente secondes sur une résolution de nom que sa
  propre protection bloquait.
- Se reconnecter tout seul après une mise à jour au lieu de rester bloqué jusqu'au clic.


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
