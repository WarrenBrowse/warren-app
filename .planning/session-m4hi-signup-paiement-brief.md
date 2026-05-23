# Session M4.H.I — Signup flow + tunnel paiement (#3)

> Brief d'agent autonome warren-core (warren-api) + warren-app (desktop) + warrenbrowse-site (frontend).
> Doctrine §0.0 INVIOLABLE + §0.5 full autonomy + §0.6 worktree séparé obligatoire.
> Grosse session : conversion bloquée jusqu'à livraison.

**Effort estimé** : wall-clock 2-3 semaines.
**Coût Hetzner** : ~0.10 EUR (smoke tests warren-backend-api dev).
**Pré-conditions** :
- warren-app `main` HEAD `eced6c8613+`
- warren-core `main` HEAD `fed1c88+`
- warrenbrowse-site `main` (Astro 5 + Tailwind + i18n FR/EN, livré M4.H.H)
- BTCPay self-host instance accessible (escalation case 5 si pas dispo)

**Objectif** : transformer warrenbrowse.com d'un site marketing avec CTA mailto: placeholder en tunnel de conversion complet (signup + paiement crypto + carte + cash). Stocker subscription state warren-api SQLite. Wallet Ed25519 = identité auth (non-custodial, parité desktop session B onboarding).

Sous-phases (séquentielles autonomes) :

1. **M4.H.I.1 — Setup worktree multi-repo + décision provider paiement** (~1j)
2. **M4.H.I.2 — Backend warren-api subscription store + checkout webhook handlers** (~5-7j)
3. **M4.H.I.3 — BTCPay self-host integration (Monero + BTC + Lightning)** (~3-5j)
4. **M4.H.I.4 — Stripe integration (carte EU)** (~2-3j)
5. **M4.H.I.5 — Cash by mail handler (Mullvad-style)** (~1-2j)
6. **M4.H.I.6 — Frontend signup form Astro (warrenbrowse-site)** (~3-5j)
7. **M4.H.I.7 — Frontend checkout flow + redirect provider** (~2-3j)
8. **M4.H.I.8 — Account dashboard mini (renewal + expiry)** (~1-2j)
9. **M4.H.I.9 — Tests E2E cross-stack + rapport** (~2-3j)

---

## 0.0 INVIOLABLE — pas de commande git destructive

Cf. doctrine standard.

---

## 0.5 MANDAT D'AUTONOMIE

Plein mandat. Escalade SEULEMENT si :
1. Secret leak (Stripe keys, BTCPay API, wallet keys)
2. Coût > 0.50 EUR (allocation augmentée pour cette session, BTCPay micro-payment tests)
3. Breaking change /v1 wire format warren-core
4. Signing key prod touchée
5. **Spécifique M4.H.I — DÉCISIONS BUSINESS POKA** :
   - Provider final (BTCPay self-host ? Stripe ? Cash by mail uniquement ?) — escalade obligatoire avant M4.H.I.3
   - Pricing exact (placeholders site = 7.99/6.67/5.99 EUR, à confirmer poka)
   - KYC requirements (recommendation : aucun pour anonymous payment, email opt-in pour invoice+recovery)
   - Refund period (14j légal EU minimum)
   - Account model (wallet Ed25519 = identité unique, email opt-in optional pour recovery)
   - DPO compliance GDPR billing data

Décisions tactiques agent autorisées :
- Stack frontend signup : Astro Forms vs Vue/React island
- Backend signup endpoint design (REST POST /v1/signup vs GraphQL)
- Webhook signature verification (HMAC-SHA256 standard)
- Email provider : SendGrid / Postmark / SES (recommendation Postmark, GDPR-friendly EU)
- Anti-bot : Cloudflare Turnstile (cf. CF Pages deployment M4.H.H) + rate-limit /v1/signup

---

## 0.6 WORKTREE SÉPARÉ OBLIGATOIRE

```bash
cd /Users/poka/dev/warrenBros/warren-app
git worktree add ../warren-app-m4hi main
cd /Users/poka/dev/warrenBros/warren-core
git worktree add ../warren-core-m4hi main
cd /Users/poka/dev/warrenBros/warrenbrowse-site
git worktree add ../warrenbrowse-site-m4hi main
```

Cleanup en fin :
```bash
git worktree remove ../warren-app-m4hi
git worktree remove ../warren-core-m4hi
git worktree remove ../warrenbrowse-site-m4hi
```

---

## 1. Setup initial

```bash
cd /Users/poka/dev/warrenBros/warren-core
git worktree add ../warren-core-m4hi main
cd ../warren-core-m4hi

# Read existing signup design + audit
cat docs/AUDIT-2026-05-21.md | grep -i 'signup\|subscription\|payment\|btcpay\|stripe'
ls crates/warren-api/src/handlers.rs

# Read warren-app pricing/checkout patterns desktop
cat /Users/poka/dev/warrenBros/warren-app/desktop/packages/mullvad-vpn/src/renderer/components/views/* 2>/dev/null | grep -i 'account\|expire\|subscribe' | head

# Read warrenbrowse-site pricing page
cat /Users/poka/dev/warrenBros/warrenbrowse-site/src/pages/pricing.astro
```

---

## 2. M4.H.I.1 — Setup worktree + décision provider paiement (~1j)

### Scope

1. ESCALADE OBLIGATOIRE POKA (§0.5 case 5) :
   - Provider mix paiement final : BTCPay self-host (Monero + BTC + Lightning) + Stripe (carte EU) + Cash by mail Mullvad-style ?
   - Pricing exact : 7.99 € / 39.99 € (6.67/mo) / 71.88 € (5.99/mo) confirmé ou ajusté ?
   - KYC requirements : aucun (anonymous default) ? Email opt-in recovery ?
   - Refund period : 14j EU minimum ou 30j Mullvad-style ?
   - Account model : wallet Ed25519 = identité ; email opt-in pour invoice + recovery ?
   - BTCPay self-host instance URL + API token disponibles ?
   - Stripe account + API keys (test mode + live mode) prêts ?
   - Mailbox cash by mail (adresse postale receveur) si Cash supporté
2. Documenter décisions dans `.planning/m4hi-decisions.md`

### Critères GO M4.H.I.1

- Toutes décisions business poka actées + documentées
- Provider URLs + API tokens dispo OR fallback documenté

---

## 3. M4.H.I.2 — Backend warren-api subscription store + webhook handlers (~5-7j)

### Scope

1. SQLite schema (cf. memory `warren_storage_migration` SQLite définitif) :
   ```sql
   CREATE TABLE subscriptions (
       id TEXT PRIMARY KEY,  -- UUID v4
       wallet_pubkey TEXT NOT NULL UNIQUE,  -- Ed25519 hex
       email_opt_in TEXT,  -- nullable, anonymous default
       plan TEXT NOT NULL,  -- '1m' / '6m' / '1y'
       price_eur REAL NOT NULL,
       payment_method TEXT NOT NULL,  -- 'monero' / 'btc' / 'lightning' / 'card' / 'cash'
       payment_status TEXT NOT NULL,  -- 'pending' / 'confirmed' / 'expired' / 'refunded'
       payment_external_id TEXT,  -- BTCPay invoice id / Stripe payment intent / cash code
       starts_at INTEGER NOT NULL,
       expires_at INTEGER NOT NULL,
       created_at INTEGER NOT NULL,
       updated_at INTEGER NOT NULL
   );
   ```
2. SubscriptionStore trait + SqliteSubscriptionStore impl (pattern existant `enrollment.rs`)
3. REST endpoints warren-api :
   - `POST /v1/signup` : create pending subscription, return `signup_id` + payment URL
   - `GET /v1/signup/:id` : poll signup status (pending → confirmed)
   - `POST /v1/webhook/btcpay` : BTCPay invoice update webhook (HMAC verify)
   - `POST /v1/webhook/stripe` : Stripe payment_intent.succeeded webhook (signature verify)
   - `POST /v1/admin/cash/:wallet_pubkey/confirm` : admin-side cash receipt manual confirm
4. Tests TDD strict 6+ tests :
   - Create signup pending OK
   - Webhook BTCPay confirm → status confirmed + activate exit allowlist
   - Webhook Stripe confirm → status confirmed
   - Webhook signature invalid → 401
   - Expired pending signup → cleanup task
5. Wire `warren-api allowlist` consume `subscriptions.payment_status = 'confirmed'` + `expires_at > now`

### Critères GO M4.H.I.2

- Schema migration auto warren-api boot
- 4 endpoints opérationnels
- 6+ tests PASS
- Wire allowlist functional

---

## 4. M4.H.I.3 — BTCPay self-host integration (~3-5j)

### Scope

1. BTCPay Server API client (Rust crate `btcpay_client` ou direct HTTP via `reqwest`)
2. Sur signup pending : create BTCPay invoice with amount + currency + redirect URL + webhook URL
3. Configure BTCPay Server poka-side : create store + add Monero + BTC + Lightning processors
4. Webhook handler vérifie signature HMAC-SHA256 + update subscription status
5. Tests integration avec mock BTCPay server (`mockito` crate)

### Critères GO M4.H.I.3

- BTCPay client wired
- Webhook signature verify PASS
- Tests mock PASS
- E2E smoke via BTCPay sandbox

### Décisions tactiques M4.H.I.3

- Self-host vs BTCPay-cloud : self-host (no-KYC alignment + zero fees, ops poka)
- Si self-host pas dispo : fallback BTCPay-cloud Greenfield instance (peut être démarré quick) OR différer ce provider Phase 2

---

## 5. M4.H.I.4 — Stripe integration (~2-3j)

### Scope

1. Stripe Rust crate `async-stripe`
2. PaymentIntent flow : confirm + webhook
3. Webhook signature verify via `stripe-signature` header
4. Tests integration via `stripe-mock` ou unit tests avec stub

### Critères GO M4.H.I.4

- Stripe client wired
- Webhook handler tested
- E2E smoke via Stripe test mode

### Décisions tactiques M4.H.I.4

- Connect platform vs direct account : direct (warrenBrowse SRL est marchand)
- TVA EU : Stripe Tax si subscription EU (auto-calcul) ou hors scope POC (escalation case 5)

---

## 6. M4.H.I.5 — Cash by mail handler (~1-2j)

### Scope

1. Sur signup payment_method=cash : générer cash code 12 char unique (collision-resistant via Uuid)
2. Display code à user + adresse postale receveur
3. Admin endpoint `POST /v1/admin/cash/:code/confirm` : admin marque receipt manuel
4. Email opt-in : envoi code par email avec adresse postale (template Postmark)

### Critères GO M4.H.I.5

- Cash flow opérationnel
- Admin manual confirm endpoint
- Code generation collision-resistant

---

## 7. M4.H.I.6 — Frontend signup form Astro (~3-5j)

### Scope

warrenbrowse-site (Astro 5) :
1. Page `/signup` + `/fr/signup` :
   - Form fields : plan select + email opt-in + Turnstile captcha
   - POST `/v1/signup` warren-api (CORS configured)
   - Redirect to payment provider URL
2. Composants :
   - `SignupForm.astro` (Server Island)
   - `PlanSelector.tsx` (Vue or React island for interactive)
   - `TurnstileWidget.astro` (Cloudflare Turnstile script)
3. i18n FR + EN messages
4. A11y : `role="form"`, labels, focus management

### Critères GO M4.H.I.6

- Page signup fonctionnelle + i18n
- Form submission → API call OK
- Redirect provider OK

### Décisions tactiques M4.H.I.6

- Server-side validation vs client-side : both (defense in depth)
- Recaptcha vs Turnstile : Turnstile (no Google tracking, CF Pages aligned)

---

## 8. M4.H.I.7 — Frontend checkout flow + redirect provider (~2-3j)

### Scope

1. Page `/checkout/:signup_id` : poll API, display payment provider iframe OR redirect link
2. Poll interval : 5s (avec exponential backoff jusqu'à 30s)
3. Status display : pending → confirmed (success page) ou expired (retry)
4. Pour cash flow : display code + adresse postale

### Critères GO M4.H.I.7

- Checkout flow poll + redirect OK
- Status pages success/expired

---

## 9. M4.H.I.8 — Account dashboard mini (~1-2j)

### Scope

Warren desktop app Electron : nouveau view `AccountView.tsx`
- Subscription status + plan + expires_at
- "Renew" CTA → redirect warrenbrowse.com/signup pre-filled wallet_pubkey
- "View receipt" link → BTCPay invoice / Stripe receipt

### Critères GO M4.H.I.8

- AccountView Electron livré
- Renew CTA wired
- i18n FR + EN

---

## 10. M4.H.I.9 — Tests E2E cross-stack + rapport (~2-3j)

### Scope

1. Tests integration E2E cross-stack :
   - Smoke signup BTCPay → webhook → subscription confirmed → allowlist activate → connect Warren VPN
   - Smoke signup Stripe → idem
   - Smoke signup cash → admin confirm → idem
2. Test signature invalide webhook → 401 + alert log
3. Test expired pending → cleanup task
4. Documentation runbook ops poka : confirm cash receipts admin workflow
5. Rapport `.planning/session-m4hi-report.md`
6. Memory `warren_session_m4hi_delivered.md`
7. Update MEMORY.md

### Critères GO M4.H.I.9

- 3 flows E2E PASS
- Tests sécurité webhook PASS
- Runbook ops rédigé
- Memory updated

---

## 11. Sources cross-repo à lire (PARALLÈLE)

- `crates/warren-api/src/handlers.rs` (existing endpoints)
- `crates/warren-api/src/main.rs` (axum routing)
- `crates/warren-api-types/src/lib.rs` (DTOs)
- `crates/warren-identity/src/lib.rs` (Ed25519 wallet)
- `warrenbrowse-site/src/pages/pricing.astro` (placeholders to wire)
- `desktop/packages/mullvad-vpn/src/renderer/components/views/` (UI patterns)
- Memory `warren_storage_migration` (SQLite définitif)
- BTCPay API docs https://docs.btcpayserver.org/API/Greenfield/v1/
- Stripe API docs https://stripe.com/docs/api

---

## 12. Critères GO ULTIMATE M4.H.I

- ✅ M4.H.I.1-M4.H.I.9 critères GO PASS
- ✅ Backend subscription store + 3 providers wired
- ✅ Frontend signup + checkout fonctionnels
- ✅ Account dashboard Electron livré
- ✅ Tests E2E 3 flows PASS
- ✅ Sécurité webhook signature verify
- ✅ `cargo test --workspace` warren-core PASS + clippy strict
- ✅ Astro `npm run build` warrenbrowse-site PASS
- ✅ Electron `npm test` warren-app desktop PASS
- ✅ Rapport + memory rédigés
- ✅ Worktrees cleaned

Verdict GO PARTIEL si :
- 1 provider deferred (ex: cash by mail Phase 2)
- BTCPay self-host non dispo → BTCPay-cloud fallback

---

## 13. Doctrine

- §0.0 INVIOLABLE git
- §0.5 autonomy + escalades business obligatoires §0.5 case 5
- §0.6 worktree (3 worktrees pour 3 repos)
- English-only code comments
- Pas em-dash
- Pas secrets in commits (API keys Stripe, BTCPay, Postmark)
- GDPR compliance : minimal data collection, opt-in email

---

## 14. Memory updates

- `warren_session_m4hi_delivered.md`
- Update MEMORY.md cross-repo

---

## 15. Commencer maintenant

Worktrees §0.6, ESCALADE POKA case 5 IMMEDIATE pour décisions business (M4.H.I.1). Push au fil de l'eau.

Bonne route.
