# Session Account-Dashboard — Desktop subscription dashboard (#8)

> Brief d'agent autonome warren-app desktop Electron.
> Doctrine §0.0 INVIOLABLE + §0.5 full autonomy + §0.6 worktree séparé obligatoire.
> Session courte UX : dashboard subscription post-M4.H.I.

**Effort estimé** : wall-clock 3-5 jours.
**Coût Hetzner** : 0 EUR.
**Pré-conditions** :
- warren-app `main` HEAD `eced6c8613+`
- **Session M4.H.I livrée** (subscription store + endpoints + signup tunnel) — bloquant strict
- M4.H.I.8 mini-dashboard livré déjà (renewal + expiry minimal) — cette session le finalise/étend

**Objectif** : finaliser dashboard subscription Electron desktop : status complet + payment history + renewal CTA + invoice/receipt links + voucher redemption + email opt-in management.

Sous-phases (séquentielles autonomes) :

1. **Acc.1 — Setup worktree** (~30 min)
2. **Acc.2 — gRPC endpoints subscription detail + payment history** (~1j)
3. **Acc.3 — UI AccountDashboard.tsx Electron étendu** (~1-2j)
4. **Acc.4 — Voucher redemption flow** (~0.5-1j)
5. **Acc.5 — Email opt-in management + invoice download** (~0.5j)
6. **Acc.6 — Tests + rapport + cleanup** (~0.5j)

---

## 0.0 INVIOLABLE — pas de commande git destructive

Cf. doctrine standard.

---

## 0.5 MANDAT D'AUTONOMIE

Plein mandat. Escalade SEULEMENT si :
1. Secret leak
2. Coût > 0.30 EUR (n/a)
3. Breaking change /v1 wire format ou gRPC
4. Signing key prod
5. **Spécifique Acc** : si voucher system n'existe pas backend (M4.H.I.5 cash by mail = code-based mais pas "voucher" pre-paid) → escalade pour clarifier scope ou skip Acc.4

Décisions tactiques agent autorisées :
- AccountView placement : Settings → Account vs nouveau top-nav tab
- Payment history pagination (10 last vs all)
- Invoice download : redirect BTCPay/Stripe receipt URL vs PDF généré warren-api

---

## 0.6 WORKTREE SÉPARÉ OBLIGATOIRE

```bash
cd /Users/poka/dev/warrenBros/warren-app
git worktree add ../warren-app-account main
cd ../warren-app-account
```

Cleanup :
```bash
git worktree remove ../warren-app-account
```

---

## 1. Setup initial

```bash
cd /Users/poka/dev/warrenBros/warren-app
git worktree add ../warren-app-account main
cd ../warren-app-account

# Verify M4.H.I livré
cat /Users/poka/.claude/projects/-Users-poka-dev-warrenBros-warren-app/memory/warren_session_m4hi_delivered.md 2>/dev/null
ls /Users/poka/dev/warrenBros/warren-core/crates/warren-api/src/

# Existing account UI (Mullvad pattern)
find desktop/packages/mullvad-vpn/src/renderer/components -name '*ccount*' | head
```

---

## 2. Acc.2 — gRPC endpoints subscription detail + payment history (~1j)

### Scope

1. Extension `mullvad-management-interface/proto/management_interface.proto` :
   - `rpc GetWarrenSubscription(google.protobuf.Empty) returns (WarrenSubscriptionDetail);`
   - `rpc ListWarrenPayments(google.protobuf.Empty) returns (WarrenPaymentList);`
   - `rpc RedeemWarrenVoucher(WarrenVoucherRequest) returns (WarrenVoucherResponse);`
   - `rpc UpdateWarrenEmail(WarrenEmailRequest) returns (WarrenEmailResponse);`
2. Daemon handlers query warren-api `/v1/subscription/:wallet_pubkey` + `/v1/payments/:wallet_pubkey` + `/v1/voucher/redeem` + `/v1/account/email`
3. Schemas DTOs :
   - `WarrenSubscriptionDetail { plan, expires_at_unix, payment_method, status, days_remaining, auto_renew_enabled }`
   - `WarrenPaymentList { payments: [{date, amount, method, status, receipt_url}] }`

### Critères GO Acc.2

- 4 RPCs ajoutées + daemon handlers
- Schemas DTOs typés
- Tests Rust integration mock warren-api

---

## 3. Acc.3 — UI AccountDashboard.tsx Electron étendu (~1-2j)

### Scope

1. View `desktop/packages/mullvad-vpn/src/renderer/components/views/account-dashboard/AccountDashboardView.tsx`
2. Sections :
   - **Subscription status** : plan badge + expires_at (jours restants visuel coloré) + auto-renew toggle
   - **Renew CTA** : button → ouvre warrenbrowse.com/signup?wallet_pubkey=X dans browser externe
   - **Payment history** : table 10 last (date, amount, method, status, receipt link)
   - **Voucher redemption** : input code + redeem button
   - **Email opt-in** : email input + opt-in checkbox + save
3. i18n FR + EN strings
4. A11y : labels, focus management
5. Integration Redux store (subscription data + payments slice)

### Critères GO Acc.3

- View complet + i18n
- Redux store integration
- Tests RTL 6+ unit tests

---

## 4. Acc.4 — Voucher redemption flow (~0.5-1j)

### Scope

Si système voucher existant warren-api (vs cash code M4.H.I.5) :
1. UI input voucher code → call gRPC RedeemWarrenVoucher
2. Backend warren-api `/v1/voucher/redeem` : valide + extend subscription expires_at
3. Schema vouchers SQLite (new table) :
   ```sql
   CREATE TABLE vouchers (
       code TEXT PRIMARY KEY,
       amount_eur REAL NOT NULL,
       extends_months INTEGER NOT NULL,
       redeemed_by TEXT,  -- wallet_pubkey
       redeemed_at INTEGER,
       expires_at INTEGER  -- voucher itself can expire
   );
   ```
4. Admin endpoint `/v1/admin/voucher/create` : génère codes batch
5. Tests integration : redeem valid + invalid + already-redeemed

### Critères GO Acc.4

- Voucher flow E2E
- Schema migration
- Tests 4+ PASS

### Décisions tactiques Acc.4

- Si vouchers hors scope M4.H.I : skip Acc.4 + flag deferred
- Voucher format : 16-char alphanumeric (collision-resistant + readable)

---

## 5. Acc.5 — Email opt-in management + invoice download (~0.5j)

### Scope

1. Update email opt-in via gRPC `UpdateWarrenEmail` → warren-api PATCH `/v1/account/email`
2. Invoice download : link redirect vers BTCPay invoice URL ou Stripe receipt
3. Pour cash payment : no invoice (cash receipt manuel)

### Critères GO Acc.5

- Email update flow
- Invoice links functional

---

## 6. Acc.6 — Tests + rapport + cleanup (~0.5j)

### Scope

- `npm test` desktop + `cargo test --workspace` warren-app + warren-core PASS
- Rapport `.planning/session-account-dashboard-report.md`
- Memory `warren_session_account_dashboard_delivered.md`
- Update MEMORY.md
- Cleanup worktree

---

## 7. Sources cross-repo à lire (PARALLÈLE)

- `mullvad-management-interface/proto/management_interface.proto`
- `mullvad-daemon/src/management_interface.rs`
- `desktop/packages/mullvad-vpn/src/renderer/components/views/` (UI patterns)
- `crates/warren-api/src/handlers.rs` (subscription endpoints M4.H.I)
- Memory `warren_session_m4hi_delivered` (bloquant pré-cond)

---

## 8. Critères GO ULTIMATE Acc

- ✅ Acc.2-Acc.6 critères GO PASS
- ✅ 4 gRPC RPCs livrées
- ✅ AccountDashboardView complet + i18n
- ✅ Voucher flow OK ou skip documenté
- ✅ Email + invoice OK
- ✅ Tests Rust + Vitest PASS
- ✅ Rapport + memory rédigés

---

## 9. Doctrine

- §0.0 INVIOLABLE git
- §0.5 autonomy
- §0.6 worktree
- English-only code comments
- Pas em-dash
- Pas secrets in commits

---

## 10. Memory updates

- `warren_session_account_dashboard_delivered.md`
- Update MEMORY.md

---

## 11. Commencer maintenant

PRÉ-COND : Session M4.H.I livrée + subscription backend opérationnel. Sinon escalade poka.

Worktree §0.6, sources §7 en parallèle, attaque Acc.2 gRPC. Push au fil de l'eau.

Bonne route.
