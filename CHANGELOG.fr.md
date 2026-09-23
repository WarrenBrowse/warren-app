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
### Ajouté
- Demander le code du bon quand `warren account redeem` est lancé sans code, ou le lire sur
  l'entrée standard, pour qu'un script puisse le transmettre. Un code tapé sur la ligne de commande
  est visible par tous les comptes de l'ordinateur pendant que la commande s'exécute ; la commande
  l'accepte encore à cet endroit, et le signale.
- Prévenir après une connexion au forum quand le lien ouvert avait été créé pour une connexion sur
  un autre appareil. Le forum répond à un tel lien par un code à saisir ailleurs, ce qui est
  l'allure d'une connexion lancée par quelqu'un d'autre puis envoyée à vous : l'application indique
  maintenant que l'expéditeur du lien essaie de se connecter à votre place, et qu'il ne faut pas
  lui donner le code.

### Modifié
- Terminer une connexion au forum avec un code à usage unique. Le navigateur qui a ouvert la
  connexion doit maintenant présenter un code à 6 chiffres avant que le forum ne le laisse entrer.
  Quand vous approuvez sur le même appareil, l'application ouvre ce navigateur pour qu'il termine
  seul ; après un QR code ou un code de connexion saisi dans les Réglages, l'application affiche le
  code à saisir sur l'autre appareil. Un lien que quelqu'un d'autre vous envoie ne le connecte plus.
- Réserver au compte qui a installé Warren sur cet ordinateur, et aux administrateurs, la
  connexion, la déconnexion, les réglages et l'usage du portefeuille. Les autres comptes du même
  ordinateur voient toujours si le VPN est actif. L'application de bureau indique quand Warren
  appartient à un autre compte, et redemande chaque minute.
- [macOS] Installer la version en ligne de commande sous `/opt/warren` plutôt que `/usr/local`, que
  Homebrew confie au compte qui l'a installé sur les Mac Intel. L'installateur refuse un dossier
  qu'un autre compte peut modifier, et déplace une installation existante.

### Corrigé
- Se reconnecter seul quand l'emplacement choisi retrouve une sortie après une maintenance. Si la
  liste des sorties que l'application avait récupérée n'en contenait plus aucune à cet endroit, la
  connexion restait bloquée jusqu'à un clic de votre part ; l'application récupère maintenant la
  liste à nouveau et réessaie chaque minute.
- [Android, iOS] Faire passer la connexion par un autre serveur dès qu'un serveur qu'elle traverse
  annonce une maintenance ou refuse les nouvelles connexions, au lieu de le réessayer jusqu'à son
  retour.
- [Android] Le dire quand la connexion coupait sans cesse et que l'application a cessé de réessayer
  en laissant passer votre trafic par votre réseau habituel, hors du VPN, et indiquer le Mode
  verrouillage comme moyen de le garder bloqué. L'application affichait un message générique sur une
  possible fuite de trafic réseau.

### Sécurité
- Rendre le fichier de réglages lisible par les seuls administrateurs, et créer les rapports de
  problème lisibles par leur seul propriétaire.
- Durcir l'application de bureau : elle ne peut plus être lancée comme simple interpréteur de
  scripts, ignore les options de débogage et de Node sur sa ligne de commande, ne charge son code
  que depuis sa propre archive et, sous macOS et Windows, refuse de démarrer quand cette archive a
  été modifiée.
- Mettre à jour l'application de bureau vers Electron 39.8.10, ainsi que ses composants gRPC et XML,
  au-delà des vulnérabilités publiées.
- [Windows, macOS, Android] Tenir le code de connexion au forum à l'écart des captures d'écran et
  du partage d'écran tant qu'il est affiché.
- N'ouvrir aucune page de connexion au forum qui appartient à une autre connexion que celle que
  vous avez approuvée.
- Lancer les outils système dont la connexion dépend depuis leur emplacement système fixe, jamais
  depuis le chemin de recherche de celui qui a lancé l'application.
- [Linux] Lancer un programme hors du tunnel uniquement pour le compte propriétaire de Warren ou
  pour un administrateur.
- [Linux] Réserver au compte propriétaire de Warren, et aux administrateurs, la fermeture de la
  connexion VPN affichée dans le menu réseau du bureau. La fermer retirait l'adresse du tunnel.
- [Android] Connecter le VPN uniquement sur une demande de l'application elle-même. Une autre
  application pouvait envoyer la même demande et faire connecter le VPN.
- [Windows] Faire parler la ligne de commande uniquement à un canal de gestion servi par un
  administrateur, et couper une connexion de gestion restée muette trente secondes.
- [macOS] Ne jamais lancer en administrateur l'outil d'installation de la version précédente
  pendant une mise à jour, sauf si seuls les administrateurs ont pu le modifier, et garder la
  protection de mise à jour là où aucun autre compte ne peut la remplacer.
- Signer la liste des sommes de contrôle de chaque version en ligne de commande avec la clé de
  publication de Warren. Les scripts d'installation refusent un téléchargement qu'elle ne garantit
  pas, et les archives enregistrent chaque fichier comme appartenant au compte administrateur.
- Tenir l'adresse interne du tunnel à l'écart des journaux de l'application.
- Récupérer un abonnement acheté depuis l'application avec un secret que l'application garde pour
  elle et n'envoie que dans la requête qui récupère le bon, pour qu'un lien d'achat vu par quelqu'un
  d'autre ne lui donne plus le bon. Un achat commencé avec une version précédente et encore en
  attente de paiement n'est plus récupéré automatiquement.
- Quand le tunnel est reconstruit, ne redemander son ancienne adresse interne qu'à la sortie qui l'a
  attribuée.

## [1.1.31] - 2026-09-21
### Ajouté
- Se connecter sur un réseau qui ne donne aucune adresse IPv4 à votre appareil. Quatre des six
  sorties répondent maintenant aussi en IPv6, l'application choisit la famille d'adresses que votre
  réseau peut réellement joindre avant de composer, et ouvre les deux dans le pare-feu. Un réseau
  mobile en IPv6 seul laissait l'application composer une adresse hors d'atteinte, coupe-circuit
  actif.
- Donner à la commande de port forwarding une sortie lisible par une machine, pour qu'un script
  puisse piloter un client torrent comme qBittorrent sans lire le texte destiné à l'humain.
  `warren port-forward get --json` et `warren port-forward status --json` affichent un objet JSON
  par ligne, `status --wait` attend que le port public soit accordé et indique le résultat par son
  code de sortie, et `status --watch --exec` lance la commande de votre choix à chaque changement
  du port public accordé.
- Copier le port public accordé depuis l'écran de port forwarding. Un bouton de copie est placé à
  côté de chaque port ouvert et met le seul numéro de port dans le presse-papiers, prêt à coller
  dans le champ du port d'écoute d'un client torrent.
- Vous prévenir quand un port public accordé s'ouvre, change ou se ferme, par une notification
  système qui nomme le nouveau port et ouvre l'écran de port forwarding. Un renouvellement qui
  garde le même port ne dit rien. La notification se désactive depuis l'écran de port forwarding.
- Publier les ports publics accordés dans un fichier qu'un script peut surveiller, un port par
  ligne, écrit à côté des réglages de l'application sous le nom `forwarded_port`
  (`~/.config/Warren VPN/` sous Linux, `~/Library/Application Support/Warren VPN/` sous macOS,
  `%LOCALAPPDATA%\Warren VPN\` sous Windows). Le fichier se vide quand le tunnel est coupé, et
  il est réécrit sur place pour qu'une surveillance continue de le suivre.
- Saisir le port public accordé dans votre client torrent, et l'y maintenir. Choisissez
  qBittorrent, Transmission ou Deluge sur l'écran de port forwarding, donnez l'adresse de son
  interface web et ses identifiants, et l'application écrit le port accordé dans le client via
  l'API de ce client, à chaque fois que le serveur de sortie déplace le port. Elle désactive aussi
  le tirage de port aléatoire et l'UPnP du client, qui le ramèneraient aussitôt hors du port
  accordé. « Tester la connexion » rapporte la réponse du client et « Appliquer maintenant » écrit
  le port courant sans attendre. Le mot de passe est conservé dans le trousseau du système
  d'exploitation, jamais dans le fichier de réglages.
- Relier l'écran de port forwarding au guide du site expliquant comment configurer un client
  torrent avec un port accordé.

### Corrigé
- Vous dire pourquoi votre trafic est bloqué quand votre réseau ne peut pas joindre Warren du
  tout. L'application attendait en silence un réseau utilisable, donc un téléphone qui avait
  internet restait derrière le coupe-circuit sans explication à l'écran. Elle nomme désormais la
  famille d'adresses manquante et indique les deux sorties: se déconnecter, ou changer de réseau.
- Empêcher une tentative de connexion de rester bloquée vingt secondes sur un réseau qui laisse le
  tunnel démarrer puis le coupe. L'application demandait son adresse au serveur et attendait une
  réponse qui n'arrivait jamais, ce qui consommait toute la tentative sur cette seule attente, sans
  jamais atteindre le moment où elle bascule en TCP. Elle abandonne maintenant une réponse
  silencieuse au bout de six secondes, et envoie la tentative suivante en TCP sur le port 443
  directement.
- Conserver un fichier de journal de plus sur Android. Rouvrir l'application deux fois après une
  connexion échouée effaçait jusqu'ici les traces que l'on cherche justement dans un rapport de bug.
- Empêcher la carte de connexion Android de repasser au vert après que le système a repris le VPN.
  Quand une autre application prenait la place du VPN pendant qu'une connexion était encore en
  cours, le tunnel était bien démonté, mais une mise à jour d'état déjà en route réaffichait
  « Connecté » sur une session qui n'existait plus.
- Afficher les notes de version de l'application installée dans la langue de l'application. L'écran
  « Quoi de neuf » lisait un fichier unique, écrit en anglais, donc une application en français ou
  en roumain annonçait ses propres changements en anglais. Les notes proposées avec une mise à jour
  étaient déjà traduites.


## [1.1.30] - 2026-09-12
### Corrigé
- Empêcher le tunnel d'accumuler plusieurs secondes de file d'attente sur un lien d'envoi lent. Le
  tampon d'envoi se réduisait déjà vers ce que la connexion peut réellement transporter, mais il ne
  pouvait jamais descendre sous un plancher dimensionné pour une ligne rapide : sur un envoi autour
  de 1 Mbit/s, le plancher était donc le tampon, et il retenait environ neuf secondes de trafic.
  Une connexion reçoit maintenant un plancher dimensionné pour un lien domestique, ce qui supprime
  ce délai sans rien changer sur une ligne plus rapide.


## [1.1.29] - 2026-09-09
### Ajouté
- Joindre les journaux de l'application à un rapport de bug du forum depuis Android, à partir du
  même lien du forum que l'application de bureau traite déjà. Une demande de consentement nomme le
  sujet concerné, affiche sur demande le rapport expurgé exact, et n'envoie qu'après approbation.
  Un code de session issu de cette page, saisi là où va un code de connexion au forum, ouvre la
  même demande au lieu d'être lu comme une connexion expirée.
### Corrigé
- Afficher l'icône de l'application dans le dock et le sélecteur de fenêtres de GNOME sur une
  session Wayland (Ubuntu 26.04). L'application Linux ne nommait jamais son entrée de bureau au
  compositeur, donc GNOME ne pouvait pas associer la fenêtre au lanceur installé et affichait
  l'icône générique.
- Laisser l'application basculer sur son transport TLS-over-TCP pendant que le tunnel est monté.
  Sur un réseau qui bloque ou tue UDP, l'application doit transporter le tunnel en TCP à la place,
  et sur macOS le kill switch n'autorisait que UDP vers le relais : le repli ne pouvait donc jamais
  être appelé et l'application se reconnectait en boucle. Elle atteint maintenant le même relais
  sur le même port avec l'un ou l'autre protocole.
- Se reconnecter tout de suite quand le socket de transport macOS est mesuré comme trou noir juste
  après la connexion, au lieu de laisser le tunnel mort en place pendant 30 secondes. La première
  connexion sur un réseau que l'application ne connaît pas lie ce socket à l'interface physique et
  vérifie en deux secondes que les paquets reviennent ; quand rien ne revient, la vérification met
  fin à la session immédiatement, et la reconnexion qui suit emprunte l'échappement par route
  d'hôte déjà enregistré. Jusqu'ici le même constat attendait deux garde-fous de 15 secondes, la
  machine entière coupée d'internet pendant ce temps.



## [1.1.28] - 2026-09-07
### Modifié
- Utiliser d'abord le transport TLS-over-TCP sur un réseau qui laisse le tunnel s'établir puis le
  tue. Certains réseaux laissent passer le handshake et coupent la session vingt secondes plus
  tard, et l'application relançait la course UDP à chaque tentative : elle se reconnectait en
  boucle sans rien transporter. Après deux sessions de ce type d'affilée, elle appelle maintenant
  le transport TCP en premier pendant quinze minutes, et revient à UDP dès qu'une session tient.
### Corrigé
- Activer l'accès beta dès la création du compte, et non seulement à la fin de l'assistant de
  configuration. Un premier lancement interrompu avant cette étape laissait tous les écrans
  annoncer un compte arrivé à expiration, alors qu'il n'avait jamais été enregistré.
- Garder la fenêtre de l'application à l'écran pendant le premier lancement. Elle se cachait dès
  que le focus passait ailleurs, emportant la phrase de récupération et l'étape d'activation avant
  qu'on puisse les lire.
- Ne plus signaler qu'un compte beta est arrivé à expiration quand il n'a jamais été activé. Le
  message se lisait comme un compte mort et poussait à désinstaller l'application.


## [1.1.27] - 2026-09-05
### Ajouté
- Publier aussi chaque installateur en BitTorrent. La page de téléchargement porte maintenant un
  fichier .torrent et un lien magnet à côté de chaque fichier, avec un seed permanent et un tracker
  annoncé, pour que Warren reste joignable quand le serveur de téléchargement est bloqué.


## [1.1.26] - 2026-09-04
### Modifié
- Donner à l'application beta ses propres couleurs dans la barre système, orchidée quand elle est
  déconnectée et azur quand elle est connectée, pour qu'un ordinateur faisant tourner la beta et la
  version de production n'affiche jamais deux icônes identiques.
### Corrigé
- [Windows] Corriger la disparition de l'icône dans une session de bureau à distance qui réduit le
  nombre de couleurs. Les copies en couleurs réduites contenues dans les fichiers d'icônes
  monochromes étaient vides.


## [1.1.17] - 2026-08-16
### Modifié
- [macOS] Essayer un autre serveur d'entrée quand une connexion ne transporte rien et que
  l'ordinateur a été mis hors de cause. Quand le trafic cesse d'atteindre le tunnel, deux choses
  peuvent l'expliquer : l'ordinateur n'émet pas, ou ce serveur précis est injoignable depuis le
  réseau utilisé. L'application ne traitait que la première, en réessayant indéfiniment le même
  serveur. Elle vérifie maintenant si ses propres paquets quittent réellement l'ordinateur, et
  quand c'est le cas elle demande un autre serveur d'entrée au lieu de répéter une tentative qui ne
  peut pas aboutir.
### Corrigé
- [macOS] Consigner ce qu'une connexion en échec a réellement mesuré. Quand l'application signale
  que le trafic ne peut pas quitter l'ordinateur, le rapport qu'elle joint indique désormais
  combien elle a émis, combien lui est revenu, combien de connexions étaient utilisées, et si le
  système ferait sortir le trafic du tunnel par l'interface réseau ou le renverrait dans le tunnel.
  Sans cela, le support ne pouvait pas distinguer un ordinateur qui n'émet plus d'un réseau qui ne
  transporte plus, et la personne qui signalait le problème non plus.

## [1.1.16] - 2026-08-15
### Corrigé
- [macOS] Ne plus laisser un ordinateur sans résolution de noms après une déconnexion. Quand un
  autre VPN tournait, l'application pouvait enregistrer l'adresse du résolveur privé de ce VPN comme
  le réglage DNS d'origine de l'ordinateur, puis le restaurer à la déconnexion. Cette adresse meurt
  avec le programme qui la détenait, et le réglage dans lequel elle était écrite est une
  configuration utilisateur permanente qu'aucun changement de réseau ni redémarrage ne réécrit,
  si bien que l'ordinateur pointait vers le vide jusqu'à une reconfiguration manuelle du DNS.
  L'application reconnaît désormais une telle adresse comme la surcharge d'un VPN, ne l'enregistre
  jamais comme un réglage d'origine, et efface celles qu'elle trouve, quel que soit le programme
  qui les a laissées.
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
