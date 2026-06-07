# Plan stratégique — Test E2E du flow d'achat forfait Warren (Stripe test mode)

> Objectif : tester de bout en bout, depuis un onboarding vierge, l'achat d'un
> forfait Warren via Stripe (cartes de test 4242), l'obtention du voucher, et
> son activation dans l'app. Backend lançable en mode test OU prod via env var.

## 0. État des lieux (investigation)

### Ce qui existe déjà (backend `warren-core/crates/warren-api`)
Le modèle anti-corrélation voucher est **entièrement implémenté** :

| Étape | Endpoint | Fichier |
|---|---|---|
| Stripe paie | `POST /webhook/stripe` (HMAC-SHA256, anti-replay 300s, dédup) | `providers/stripe.rs`, `handlers/payment.rs` |
| Voucher anonyme créé (keyed sur `payment_intent.id`) | interne → `pending_vouchers` (TTL 24h) | `handlers/payment.rs:177` |
| Payeur récupère le code | `GET /v1/checkout/{pending_id}/voucher` (no-auth, single-use) | `handlers/payment.rs:421` |
| App active le voucher | `POST /v1/register` (daemon `submitVoucher`) | `handlers/subscription.rs:199` |
| App lit l'expiry | `GET /v1/subscription` | `handlers/subscription.rs:32` |
| Montant→durée | table `pricing_tier` (EUR/USD) | `pricing_pg.rs` |

`pending_id` = `data.object.id` = l'ID du PaymentIntent (`pi_...`).

### Ce qui existe déjà (app desktop `warren-app/desktop`)
- Onboarding `Welcome → Wallet (mnemonic Ed25519) → Subscription`.
  `OnboardingSubscriptionView` ouvre `urls.pricing` dans le navigateur puis poll
  `/v1/subscription` toutes les 10s pendant 2 min.
- UI redemption voucher `RedeemVoucher.tsx` (format Crockford-32 `XXXX-XXXX-XXXX-XXXX`).
- Setting `warren_api_url` (`WarrenApiUrlSetting.tsx`) → pointer l'app sur un backend local.

### Ce qui MANQUE (les 3 pièces à livrer)
1. **Aucun tunnel de paiement web.** `warrenbrowse-site` est un Astro **statique**
   (marketing). Aucune création de Stripe Checkout Session nulle part.
2. **Pas de toggle test/prod dans le backend.** `providers/stripe.rs` ne lit pas
   le champ `livemode` de l'événement → impossible de refuser des paiements test
   en prod, ni de les autoriser explicitement en test.
3. **Le secret `sk_` Stripe ne doit pas vivre dans warren-api** (design anti-PII :
   warren-api ne parle jamais à l'API Stripe, il vérifie seulement des webhooks).
   → la création de session vit côté web SvelteKit.

## 1. Architecture cible

```
                         ┌─────────────────────────────────────────┐
  Warren desktop app     │   warren-checkout (SvelteKit SSR, NEW)   │
  ┌───────────────┐      │  /pricing  -> POST /api/checkout (sk_*)  │
  │ Onboarding    │  (1) │            -> Stripe Checkout (hosted)   │
  │  Subscription │──────┼──> navigateur                            │
  │  "View plans" │      │  /success?session_id=...                 │
  └───────┬───────┘      │     retrieve session -> payment_intent   │
          │              │     poll GET /v1/checkout/{pi}/voucher   │
          │              │     affiche VOUCHER + copier             │
          │              └────────────────┬────────────────────────┘
          │   (4) coller voucher           │ (2) card 4242
          │                                 v
          │                          ┌─────────────┐
          │                          │   Stripe    │ (test mode)
          │                          └──────┬──────┘
          │                       (3) webhook payment_intent.succeeded
          │                                 v
          │              ┌──────────────────────────────────────┐
          └──submitVoucher─> warren-api (warren-core)            │
             POST /v1/register │  /webhook/stripe (livemode gate) │
                               │  pending_vouchers -> voucher     │
                               │  subscriptions (expiry)          │
                               └───────────┬──────────────────────┘
                                           v  Postgres 16 (docker)
```

Le mode **test vs prod** est déterminé par DEUX leviers indépendants mais alignés :
- **Côté web** : `STRIPE_SECRET_KEY` = `sk_test_…` (test) ou `sk_live_…` (prod).
- **Côté backend** : `WARREN_STRIPE_ALLOW_TEST_MODE=1` accepte les events
  `livemode:false` ; sinon (prod par défaut) ils sont ignorés (ack 200, pas de
  voucher). Les events test sont de toute façon signés avec le `whsec` de
  l'endpoint test, donc défense en profondeur.

## 2. Phase A — Backend : toggle test/prod via env var (warren-core)

**Fichiers** : `crates/warren-api/src/providers/stripe.rs`, `config.rs`.

1. `StripeEvent` : ajouter `#[serde(default)] livemode: bool` (les vrais events
   Stripe le portent toujours ; défaut `false` = traité comme test).
2. `StripeHandler` : champ `allow_test_mode: bool`, `new()` défaut `false`,
   builder `with_test_mode(bool)`.
3. `parse_event` : après les filtres type/status, si `!livemode && !allow_test_mode`
   → `WebhookError::Ignored { reason: "test-mode event rejected (set WARREN_STRIPE_ALLOW_TEST_MODE)" }`.
4. `config.rs` :
   - `StripeProviderConfig` : `#[serde(default)] pub allow_test_payments: Option<bool>`.
   - Helper pur `resolve_bool_flag(env, toml, default)` (généralise
     `resolve_roster_enabled`, env-wins-then-TOML-then-default).
   - `into_app_state` : résoudre via `WARREN_STRIPE_ALLOW_TEST_MODE` puis
     `.with_test_mode(allow)` ; `tracing::warn!` au boot si actif.
5. Tests :
   - MAJ fixtures unitaires existantes (ajout `livemode`) pour rester GREEN.
   - Nouveaux : test-event rejeté quand non-autorisé ; accepté quand autorisé ;
     live-event accepté quel que soit le flag ; `resolve_bool_flag` precedence.
   - MAJ tests intégration `webhook_voucher_e2e.rs` / `webhook_dedup.rs`
     (ajouter `livemode` ou `with_test_mode(true)`).

**Critère GO A** : `cargo test -p warren-api` GREEN + clippy clean.

## 3. Phase B — Tunnel de paiement `warren-checkout` (SvelteKit SSR)

**Nouveau dossier** : `warrenBros/warren-checkout/` (SvelteKit + adapter-node +
TS + Tailwind, Stripe node SDK).

Routes :
- `GET /` ou `/pricing` : sélecteur de plan (30j / 365j), montants alignés sur
  `pricing_tier` (ex. 1000 cents EUR → 30j, 5000 → 365j). Bandeau **TEST MODE**
  affiché si `STRIPE_SECRET_KEY` commence par `sk_test`.
- `POST /api/checkout` (server) : `stripe.checkout.sessions.create({ mode:'payment',
  line_items:[{price_data:{currency, unit_amount, product_data}, quantity:1}],
  success_url:'/success?session_id={CHECKOUT_SESSION_ID}', cancel_url:'/cancel' })`.
- `GET /success` (server load) : `sessions.retrieve(session_id)` →
  `session.payment_intent` → poll `GET {WARREN_API_BASE_URL}/v1/checkout/{pi}/voucher`
  (backoff jusqu'à ~60s) → afficher le voucher `XXXX-XXXX-XXXX-XXXX` + bouton
  copier + consignes "coller dans l'app Warren".
- `GET /cancel` : message + retour pricing.

Env (`.env` / `.env.example`) :
`STRIPE_SECRET_KEY`, `PUBLIC_STRIPE_PUBLISHABLE_KEY`, `WARREN_API_BASE_URL`,
`PUBLIC_BASE_URL`. Aucune clé live commitée.

UX : moderne, responsive, i18n FR/EN minimal, états loading/erreur/succès soignés,
a11y (labels, focus). Design aligné Warren (réutiliser tokens/couleurs du site).

**Critère GO B** : `npm run build` OK ; `/pricing` → Checkout test → `/success`
affiche un voucher récupéré du backend local.

## 4. Phase C — Orchestration locale e2e

**Fichier** : `warren-core/scripts/dev-e2e-stripe.sh` (+ `warren-api.test.toml`).

Étapes scriptées :
1. `docker compose up -d postgres` (warren-core).
2. Seed admin pubkey (depuis mnemonic dev) + pricing tiers (via `/v1/admin/pricing`
   ou SQL direct).
3. `stripe listen --forward-to localhost:8080/webhook/stripe` → fournit le `whsec`
   test → injecté dans `warren-api.test.toml` (`[providers.stripe] webhook_secret`,
   `allow_test_payments = true`).
4. Lancer `warren-api` avec `WARREN_STRIPE_ALLOW_TEST_MODE=1 --config warren-api.test.toml`.
5. Lancer `warren-checkout` (`sk_test_…`, `WARREN_API_BASE_URL=http://localhost:8080`).
6. Pointer l'app desktop sur `http://localhost:8080` via setting `warren_api_url`
   (ou `urls.pricing` override → `http://localhost:5173/pricing`).

Documente prérequis : compte Stripe test, `stripe` CLI, cartes test (4242…,
décline 4000 0000 0000 0002, 3DS 4000 0025 0000 3155).

**Critère GO C** : un seul script amène toute la stack test debout.

## 5. Phase D — Walkthrough E2E + automatisation

1. Walkthrough manuel documenté (`warren-checkout/E2E.md`) : onboarding vierge →
   générer wallet → "View plans" → checkout 4242 → voucher → coller dans app →
   `submitVoucher` → expiry actif → connexion VPN.
2. Smoke headless (`scripts/dev-e2e-stripe.sh --smoke`) : `stripe trigger
   payment_intent.succeeded` (ou checkout réel via API) → `curl` pull voucher →
   `curl POST /v1/register` → `GET /v1/subscription` montre l'expiry.
3. (Option) Playwright sur `warren-checkout` (`/pricing` → mock Stripe → `/success`).

**Critère GO D** : smoke headless PASS, walkthrough manuel validé.

## 6. Risques & décisions

- **Anti-PII préservé** : `sk_` reste côté web ; warren-api ne voit que le webhook.
- **Checkout `payment_intent.succeeded`** : en mode `payment`, Stripe émet bien cet
  event ; `session.payment_intent` = même `pi_…` que `data.object.id` du webhook.
- **Montant ⇄ pricing_tier** : le `unit_amount` du line_item DOIT matcher un tier
  seedé, sinon webhook → 422 BelowMinimum, pas de voucher.
- **Backend test toggle ≠ build debug** : c'est une env var runtime (plus souple,
  permet un déploiement de staging). La sécurité prod vient du défaut OFF +
  whsec distinct.

## 7. Ordre d'exécution

A (backend toggle + tests) → B (SvelteKit) → C (orchestration) → D (walkthrough).
Commits seulement sur demande de l'utilisateur.
