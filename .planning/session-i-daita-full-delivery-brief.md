# Session H — DAITA full delivery (exit-side + multi-hop + defense in depth + bench)

> Brief autonome warren-core (surface principale) + warren-app (pin bump).
> Doctrine §0.0 INVIOLABLE git + §0.5 plein mandat + §0.6 worktree séparé obligatoire.
> Continuation directe Session G : pump client-side wired, reste exit-side + multi-hop + tests cross-cutting + bench full DAITA.

**Effort estimé** : 6-10j wall-clock.
**Coût Hetzner** : ~0.05-0.10 EUR (H.6 bench cross-DC).
**Pré-conditions** :
- warren-core `main` HEAD `30a7e3c+` (Session G fix)
- warren-app `main` HEAD `f109eb7+` (Session G pin bump)
- Session G report `.planning/session-g-report.md`

**Objectif** : DAITA totally end-to-end, défense bidirectionnelle complète, validée empiriquement Hetzner cross-DC. Closes B.1.8 caveat session B définitivement avec mesure overhead bandwidth réelle.

Sous-phases (séquentielles) :

1. **H.1 — Exit-side aggregation infra** (~2j)
2. **H.2 — Wire exit-side `pump_multi_bidirectional_with_daita`** (~1j)
3. **H.3 — Multi-hop DAITA client-side wiring** (~2j)
4. **H.4 — Dummy filter defense-in-depth cross-cutting** (~0.5j)
5. **H.5 — Integration tests end-to-end** (~1j)
6. **H.6 — Hetzner cross-DC bench full DAITA** (~0.5j)
7. **H.7 — Rapport + memory + commit + push + cleanup** (~0.5j)

---

## 0.0 INVIOLABLE — pas de commande git destructive

Cf. doctrine standard. Préserver WIP poka warren-app + warren-core. Submodules warren-core preserved. Vendor symlink session G nettoyé.

---

## 0.5 MANDAT D'AUTONOMIE

Plein mandat. Escalade `AskUserQuestion` SEULEMENT si :
1. Secret leak
2. Coût Hetzner > 0.30 EUR (H.6 cap 0.10)
3. Breaking change /v1 wire format
4. Signing key prod
5. **Spécifique H** : si exit-side aggregation nécessite breaking change protocole (e.g. nouveau frame Setup) ou refactor pump architecture, escalade pour validation archi

Décisions tactiques agent autorisées (déjà arrêtées) :
- Exit-side DaitaState **per-session** (1 par Ed25519 identity, shared across N conns) — aligned avec `daita_spec` per session existant
- Multi-hop DAITA v1 = **client-only** (defense uplink) — exit-side multi-hop = follow-up M5.B.2
- Dummy filter `pump_quic_to_tun` cross-cutting : casse le test contract intentionnel, remplacement test
- Bench H.6 stack production binaire (`warren-client --enable-daita --use-tun --num-conns N`) — pas le binaire bench-only

---

## 0.6 WORKTREE SÉPARÉ OBLIGATOIRE

```bash
cd /Users/poka/dev/warrenBros/warren-core
git fetch origin
git worktree add -b session-h-daita-full ../warren-core-daita-full origin/main
cd ../warren-core-daita-full
ln -sfn /Users/poka/dev/warrenBros/warren-core/vendor vendor   # quinn-fork local convenience
git status                                  # clean main 30a7e3c+
```

Tous les commits + push depuis ce worktree. Cleanup en fin de session.

---

## H.1 — Exit-side aggregation infra (~2j)

### Scope H.1

Construire l'analog server-side de `MultiSession` côté client : agréger N conns Quinn de la même Ed25519 identity sous une session unique avec un `DaitaState` partagé.

1. **H.1.1** Mapper l'architecture actuelle `accept_forever_with_tun` : comment N conns d'un même client sont actuellement traitées (1 pump par conn vs aggregation ?)
2. **H.1.2** Design `ServerMultiSession` : `Vec<Connection>` + `Arc<Mutex<DaitaState>>` + pump handle + lifecycle
3. **H.1.3** Mécanisme de "join secondaire" : sur handshake secondaire, comment add conn à un ServerMultiSession existant ? `Arc<DashMap<WarrenPubkey, ServerMultiSession>>` ou autre.
4. **H.1.4** TDD : test integ `exit_server_aggregates_multi_conn_session.rs`

### Critères GO H.1

- ServerMultiSession struct + lifecycle codée
- Test integ aggregates 4 conns + shared state
- `cargo test --workspace` PASS
- Commit warren-core push origin/main

---

## H.2 — Wire exit-side `pump_multi_bidirectional_with_daita` (~1j)

### Scope H.2

Refactor `accept_forever_with_tun` pour utiliser ServerMultiSession :

1. **H.2.1** Sur primary handshake : créer ServerMultiSession avec 1 conn + DaitaState from daita_spec, spawn pump_multi_bidirectional_with_daita
2. **H.2.2** Sur secondary handshake : push conn into ServerMultiSession, spawn additional downlink task (pump_multi_bidirectional_with_daita exposes N conns at spawn time — donc soit on attend tous les N avant de spawn, soit on étend l'API)
3. **H.2.3** Decision tactique : attendre N=total_connections avant de spawn pump (timeout limité)
4. **H.2.4** Rate-limit compat : la couche per-identity bucket reste fonctionnelle (apply_to chaque conn ou décoder dans le pump shared)
5. **H.2.5** Tests : exit délivre dummies en plus des paquets réels, downlink défense active

### Critères GO H.2

- accept_forever_with_tun utilise ServerMultiSession + DAITA pump
- Test integ ConnectivityFlow + dummies bidir
- `cargo test --workspace` PASS
- Commit warren-core push origin/main

---

## H.3 — Multi-hop DAITA client-side wiring (~2j)

### Scope H.3

Activer DAITA dans le path multi-hop `run_multi_hop` / `MultiHopClient`.

1. **H.3.1** Mapper la pipeline multi-hop actuelle : où le pump est-il instancié ?
2. **H.3.2** Decision tactique : MultiHopClient utilise HPKE direct sans SetupAck → option A `hardcoded DAITA config` client-side (DaitaPool.default_pool.pick) ou option B négocier via extension MultiHopSetup
3. **H.3.3** Implémentation v1 : option A pour minimiser scope (multi-hop DAITA v1 = client-only avec config par défaut)
4. **H.3.4** Tests : multi-hop pump avec DAITA active

### Critères GO H.3

- Multi-hop pump utilise DaitaState
- Test integ multi_hop_pump_with_daita PASS
- `cargo test --workspace` PASS

---

## H.4 — Dummy filter defense-in-depth (~0.5j)

### Scope H.4

Ajouter `is_daita_dummy` filter dans les pumps non-DAITA (cas où sender DAITA mais receiver disabled — défense robuste).

1. **H.4.1** Modifier `pump_quic_to_tun` + `pump_quic_to_tun_rate_limited` : filter dummies avant tun.send
2. **H.4.2** Revisiter test contract `pump_multi_with_daita_disabled_still_filters_dummy_first_byte` — soit invert l'assertion, soit supprimer (le nouveau comportement = filter universel)
3. **H.4.3** Doc le contract change dans pump.rs module doc

### Critères GO H.4

- Filtres ajoutés + tests adaptés
- Aucune régression cargo test

---

## H.5 — Integration tests end-to-end (~1j)

### Scope H.5

Vérifier que le system complet fonctionne :

1. **H.5.1** Test sustained `daita_end_to_end_bidir.rs` : client + exit + N=4 conns + sustained 60s avec DAITA actif des deux côtés, vérifie dummies émis ET reçus des deux côtés, pas de pump termination
2. **H.5.2** Multi-hop e2e : MultiHopClient + relay + exit + DAITA actif
3. **H.5.3** `cargo test --workspace --all-targets` PASS

### Critères GO H.5

- 2 nouveaux tests e2e PASS
- Aucune régression sur le workspace

---

## H.6 — Hetzner cross-DC bench full DAITA (~0.5j) (cost cap 0.10 EUR)

### Scope H.6

Valider empiriquement en production conditions :

1. **H.6.1** Pre-flight `hcloud --context warren`
2. **H.6.2** Provision 2 nodes ccx13 FSN1+NBG1 (parité session F)
3. **H.6.3** Cross-compile + deploy warren-client + warren-exit HEAD post-H.5
4. **H.6.4** Run 3 scénarios iperf3 5 min sustained :
   - TCP -P 4 num-conns 4 DAITA OFF (baseline)
   - TCP -P 4 num-conns 4 DAITA ON full (compare)
   - UDP 200M cap DAITA ON (steady-state overhead measurement)
5. **H.6.5** Verdict :
   - Overhead bandwidth = (Base - DAITA) / Base, target ≤ 15% (Mullvad claim)
   - 0 "QUIC datagram read error: downlink: timed out" WARN (vs session F = 26)
   - 0 socket EAGAIN (vs session F UDP DAITA fail)
6. **H.6.6** Cleanup nodes, cost report

### Critères GO H.6

- Overhead mesurable (steady-state UDP)
- 0 WARN sustained 5 min
- Cost ≤ 0.10 EUR
- B.1.8 caveat session B **CLOSED**

---

## H.7 — Rapport + memory + commit + push + cleanup (~0.5j)

### Scope H.7

1. Rapport `.planning/session-h-report.md` warren-app
2. Memory warren-core : `warren_daita_full_delivery.md`
3. Update warren-core MEMORY.md
4. Memory warren-app : `warren_session_h_delivered.md`
5. Update warren-app MEMORY.md
6. Pin warren-app `.warren-core-version` bump
7. Commit + push cross-repo
8. Cleanup worktree warren-core-daita-full

---

## 5. Critères GO ULTIMATE session H

- ✅ H.1-H.7 critères GO PASS
- ✅ Exit-side DAITA pump active + tests
- ✅ Multi-hop DAITA active + tests
- ✅ Dummy filter cross-cutting wired
- ✅ Hetzner bench cross-DC 5 min DAITA full = 0 WARN + overhead mesurable ≤ 15%
- ✅ B.1.8 caveat **CLOSED**
- ✅ DAITA prod desktop + mobile + multi-hop fully active end-to-end

Verdict GO PARTIAL acceptable si :
- H.6 bench overhead > 15% (escalation case 5 archi pour tuning machine pool)
- Multi-hop DAITA v1 hardcoded acceptable, negotiated DAITA = M5.B.X follow-up

Verdict NO-GO si exit-side aggregation impossible sans breaking change protocole.

---

## 6. Doctrine

- §0.0 INVIOLABLE git
- §0.5 plein mandat
- §0.6 worktree séparé
- English-only code comments
- Pas em-dash
- TDD strict
- Push warren-core au fil de l'eau
- `hcloud --context warren` exclusif H.6

---

## 7. Commencer maintenant

Worktree §0.6, exploration archi exit-side H.1.1, design H.1.2. Plein mandat §0.5.

DAITA full prod desktop + mobile + multi-hop dépend de cette session. Sans full delivery, défense incomplète (uniquement uplink session G).

Bonne route.
