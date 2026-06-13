# Session D.7, APK build/sign + Play Store internal-test upload

D.4 step 65 scaffold pour la sortie Play Store interne. Cette procédure
n'est pas automatisable dans une session Claude Code (signing keys
hors-repo + Google Play Console hors-CLI), elle est documentée ici pour
exécution par poka.

## Pré-requis (one-time)

1. **Keystore Warren**. Générer un keystore RSA 4096 25-ans :

   ```sh
   keytool -genkey -v \
     -keystore ~/.warren/warren-upload.keystore \
     -alias warren-upload \
     -keyalg RSA -keysize 4096 -validity 9125 \
     -storetype JKS
   ```

   Conserver mot de passe + keystore hors du repo. Backup chiffré
   redondant obligatoire (perte du keystore = perte de la capacité de
   pousser des updates Play Store sur l'app id `com.warrenbrowse.vpn`).

2. **Google Play Console**. Créer l'app `com.warrenbrowse.vpn` →
   Setup → App integrity → Use Play App Signing (recommandé : Google
   garde la clé "App signing key" ; on garde la "Upload key" locale via
   le keystore ci-dessus).

3. **Internal-tester group**. Setup → Testers → Create email list
   (ex: `warren-internal@warrenbrowse.com`).

## Procédure de build local

```sh
export WARREN_KEYSTORE_PATH=$HOME/.warren/warren-upload.keystore
export WARREN_KEYSTORE_PASSWORD=...
export WARREN_KEY_ALIAS=warren-upload
export WARREN_KEY_PASSWORD=...

bash android/scripts/warren-build-release.sh
```

Sortie attendue :
- `android/app/build/outputs/apk/prod/release/app-prod-release.apk`
- `android/app/build/outputs/bundle/prodRelease/app-prod-release.aab`

⚠️ Per global rule "NEVER run flutter build" : ce script utilise `./gradlew`
direct (pas Flutter), donc autorisé.

## Upload Play Store (manuel)

1. https://play.google.com/console → Warren VPN → Testing → Internal
   testing.
2. **Create new release**.
3. Upload `app-prod-release.aab`.
4. Release notes (free-form).
5. **Save** → **Review release** → **Start rollout to Internal testing**.
6. Les testeurs reçoivent la mise à jour via Play Store en ~15 min.

## Versioning

`versionCode` + `versionName` dans `android/gradle/libs.versions.toml` (ou
au niveau du module app). Pour chaque upload Play Store il faut un
versionCode strict-monotonic croissant.

## Itérations futures

- CI : passer les env vars de signing via secrets GitHub Actions et
  uploader le `.aab` via `r0adkll/upload-google-play@v1` ou
  l'API Google Play Developer.
- Versioning auto : générer le versionCode depuis `git rev-list --count
  HEAD` pour garantir la monotonicité.
- Crashlytics / Firebase : pas wiré côté Warren (analytics opt-out
  doctrine). Le crash reporting passe par le D.6 `/v1/support` flow
  utilisateur-déclenché.
