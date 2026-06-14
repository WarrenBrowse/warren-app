# Android : checklist de test e2e sur device réel

> Procédure de validation sur appareil physique pour le travail de parité
> Android (session 2026-06-14). Tout le code est écrit et compile
> (Kotlin + Rust android), mais plusieurs morceaux n'ont jamais tourné sur un
> device. Ce document liste, dans l'ordre, ce qui reste à activer et à valider
> à la main.
>
> Légende des cases : `[ ]` à faire, `[x]` validé.
>
> Pré-requis : un device Android (minSdk 28) avec biométrie enrôlée, et un
> second device sans biométrie pour les cas de repli. NDK + SDK installés.

## 0. Build et mise en service

- [ ] Pousser les commits de la session (warren-app + le commit warren-core
      `6bb7287` "protect the client UDP socket on Android").
- [ ] Coordonner le pin `warren-app/.warren-core-version` : il pointe encore
      sur l'ancien SHA. Le build local marche (path-dep vers `../warren-core`),
      mais la CI checkout warren-core au SHA pin. Bumper le pin vers un commit
      warren-core qui inclut `6bb7287` une fois le workstream concurrent
      stabilisé.
- [ ] Builder un APK debug complet (pas seulement `compileKotlin`/`cargo
      check`) : `cd android && ./gradlew :app:assembleProdDebug`. Cela
      cross-compile `warren-jni` via le NDK et package le `.so`. Confirmer que
      le build passe de bout en bout.
- [ ] Installer l'APK sur le device.

## 1. CRITIQUE : le connect single-hop fonctionne ?

Contexte : un bug latent a été corrigé (le plugin `kotlinx-serialization`
manquait sur `:app`, donc `Json.encodeToString(WarrenTunnelConfig)` aurait
levé `SerializationException` au connect). Le chemin tunnel n'a jamais tourné
sur device.

- [ ] Créer un wallet, lancer une connexion.
- [ ] Confirmer l'état "Connected" ET que le trafic passe réellement
      (charger une page, vérifier l'IP de sortie), pas seulement le label.
- [ ] Vérifier le kill switch : couper le réseau pendant la connexion et
      confirmer que le trafic est bloqué (pas de fuite) puis reprend.

## 2. Activer et valider le multi-hop

Le multi-hop est entièrement câblé (`warren-jni/src/tunnel.rs::run_multi_hop_session`)
mais inerte : le builder Kotlin ne pose pas `entryHop`.

Activation :

- [ ] `android/app/src/main/kotlin/com/warrenbrowse/vpn/app/connect/WarrenTunnelConfigBuilder.kt` :
      remplacer `entryHop = null` par la construction d'un vrai
      `WarrenTunnelConfig.EntryHop` (re-sélectionner un relais d'entrée
      distinct, comme avant le commit P1 `f20716df4c`).
- [ ] `android/lib/feature/settings/impl/.../WarrenTunnelSettingsScreen.kt` :
      réactiver le toggle multi-hop + la sélection du pays d'entrée (remplacer
      le `MultiHopIndicator` read-only). Le chip "MULTIHOP" se rallumera tout
      seul (il dérive de `entryHop != null`).
- [ ] Bumper le pin warren-core (le socket-protect `6bb7287` est requis, sinon
      le socket QUIC du multi-hop reboucle dans le TUN).

Validation :

- [ ] Activer le multi-hop, connecter, confirmer "Connected" + chip MULTIHOP.
- [ ] POINT DE VIGILANCE (adressage TUN) : le data-plane doit vraiment passer.
      L'`IpAssign` renvoyé par `setup_over_stream` peut differer du TUN fixe
      `10.66.0.x` etabli cote Kotlin. Si l'exit n'honore pas l'IP bootstrap,
      le downlink est silencieusement droppe (= "connecte mais mort").
      Vérifier explicitement qu'on charge bien des pages en multi-hop.
- [ ] Confirmer que le trafic transite entry -> exit (l'exit ne doit pas voir
      l'IP cliente).
- [ ] Cas d'echec : si la directory `/v1/multihop/directory` est absente ou
      qu'aucun relais d'entree distinct n'existe, le connect doit echouer
      proprement (fail closed), PAS retomber en single-hop silencieusement.

## 3. Activer et valider le keystore hardware (CryptoObject)

Le chemin decrypt est cable (`AndroidKeystoreWalletRepository.unlock` branche
hardware) ; il reste le chemin encrypt et le flip du flag.

Activation :

- [ ] Cabler un `CipherAuthorizer` dans `createWallet` / `importWallet` pour le
      chemin ENCRYPT (aujourd'hui ils ne prennent pas d'authorizer). Mirroir du
      chemin decrypt : construire le `Cipher` en mode ENCRYPT, l'autoriser via
      `authorizeCipher` (CryptoObject), puis `doFinal`. Cela impose un prompt
      biometrique a la creation/import de wallet.
- [ ] Passer `HARDWARE_AUTH = true` dans `AndroidKeystoreWalletRepository`.

Validation :

- [ ] Creation de wallet : doit declencher un prompt biometrique (encrypt).
- [ ] Unlock (voir la phrase, signer une requete) : doit declencher un prompt
      biometrique lie au CryptoObject.
- [ ] Ne PAS bricker l'acces : tester qu'un wallet existant (cree avant le
      flip, donc avec l'ancienne cle sans auth) reste utilisable.
- [ ] Device SANS biometrie enrolee : confirmer le repli (refus propre, pas de
      crash, pas de wallet inaccessible).
- [ ] Re-enrolement biometrique : confirmer le comportement attendu
      (`setInvalidatedByBiometricEnrollment`).

## 4. Smoke-test de l'UI (logique testee en unitaire, rendu non verifie)

- [ ] Wallet : bouton "Get subscription" ouvre bien `https://checkout.warrenbrowse.com/`.
- [ ] Wallet : "Erase wallet" (logout) avec confirmation efface bien le wallet.
- [ ] Picker de localisation : groupement par pays, recents, recherche.
- [ ] Custom lists : creer une liste, ajouter un exit ("Add to list"), retirer
      ("Remove"), supprimer la liste ("Delete list"). Verifier la persistance
      apres redemarrage de l'app.
- [ ] TOFU : le connect echoue-t-il sur un changement de cle d'exit ? Le bouton
      "Reset pinned exit keys" (reglages tunnel) debloque-t-il ?
- [ ] Problem report : le rapport embarque-t-il desormais les logs natifs
      (dump logcat via `collectReport`) ?
- [ ] DAITA, NAT-PMP (proto/port/lifetime + statut live), allow-LAN, IPv6, MTU,
      kill switch : verifier chaque toggle de bout en bout.
- [ ] Onboarding (welcome -> privacy -> backup de phrase) sur premiere install.

## 5. Nettoyage / dette (optionnel)

- [x] Corriger les 4 tests `WarrenDisconnectUseCaseTest` /
      `WarrenReconnectUseCaseTest` (echec `Method setAction in
      android.content.Intent not mocked`, PRE-EXISTANT) : FAIT via
      `mockkConstructor(Intent::class)` + stub de `setAction`. La suite `:app`
      passe desormais entierement au vert.
- [ ] Faire tourner les tests Rust `parse_config` (warren-jni `tunnel.rs`) sur
      emulateur android (ici seulement type-checkes ; le test Kotlin
      `WarrenTunnelConfigSerializationTest` execute deja la vraie serialisation).

## Reference : commits de la session

warren-app (Android/JNI) : multi-hop honnete -> mnemonique zeroizable -> tests
wire-contract + fix plugin serialization -> suppression code mort + MTU ->
bouton Get subscription -> doc keystore -> custom lists -> cfg_attr -> log
bundle (collectReport) -> TOFU pinning -> multi-hop reel (inerte) -> keystore
hardware (inerte).

warren-core : `6bb7287` protection du socket UDP multi-hop sur Android.
