# Design, Multi-hop dynamique, fleet unifié dual-rôle (annuaire signé hors-ligne)

> Design cross-repo (warren-core + warren-app). Objectif : **chaque nœud
> Warren est exit ET relai, par défaut, toujours.** Un seul fleet. Le
> client obtient l'annuaire signé **dynamiquement** (zéro
> `warren-multihop.json` manuel) et choisit deux nœuds distincts (entrée
> + sortie). Clé opérationnelle **jamais en ligne** (modèle annuaire /
> directory-authority). Statut : DESIGN, à exécuter après validation.

---

## 0. Le modèle en une image

```
                       FLEET UNIFIÉ (tous les nœuds identiques)
        ┌──────────────────────────────────────────────────────────┐
        │  node A    node B    node C    node D   ...                │
        │  chacun : { endpoint, ed25519, x25519 HPKE, dns_disabled } │
        │  chacun sait être ENTRÉE (forward) ET SORTIE (déchiffre)   │
        └──────────────────────────────────────────────────────────┘

  Circuit du client = 2 nœuds DISTINCTS tirés du fleet :
     client ──C1──▶ node B (rôle ENTRÉE : forward aveugle)
                       └──C2──▶ node D (rôle SORTIE : déchiffre + egress)
```

- **Plus de distinction relai/exit au niveau machine.** Le rôle se décide
  *par circuit*, pas par nœud. Exactement le modèle Tor (un nœud peut être
  entrée pour un circuit et sortie pour un autre).
- Le rôle qu'un nœud joue sur une connexion donnée est déjà signalé par
  l'**opaque type byte du premier datagram** (`docs/19:76-78`), donc un
  seul serveur QUIC par nœud gère les deux rôles nativement.

---

## 1. Pourquoi c'est sûr (invariant)

La propriété d'unlinkability tient à **une seule règle de sélection** :
**entrée ≠ sortie dans un même circuit.**

- Quand le node B agit comme **entrée**, il forward du ciphertext HPKE
  chiffré pour la clé x25519 du node D. **B n'a pas la clé de D** → B est
  cryptographiquement aveugle au contenu (voit l'IP client + ciphertext).
- Quand le node D agit comme **sortie**, il déchiffre avec **sa propre**
  clé x25519 → voit le plaintext + destination, mais **pas l'IP client**
  (voit l'IP de B).
- Un nœud ne détient que **sa propre** clé HPKE, jamais celle des autres.
  Donc pour un circuit donné il est soit aveugle-forward, soit
  déchiffreur, **jamais les deux**. La défense en profondeur
  « le relai ne tient pas la clé » est préservée *par circuit*.

Limite déjà assumée (`docs/19:211-215`) : un adversaire passif global, ou
une coalition entrée+sortie sous le même opérateur, peut corréler. Mitigé
par DAITA + diversité de juridiction. Le dual-rôle **ne change pas** ce
modèle de menace (Warren opère déjà tout le fleet).

**Règle de sélection durcie** : entrée ≠ sortie (obligatoire) ; et si
possible juridiction/AS différents entre les deux hops.

---

## 2. Modèle de confiance PKI (clé opérationnelle hors-ligne)

| Clé | Emplacement | Rôle |
|---|---|---|
| **Root** | hors-ligne froid | signe la clé opérationnelle (cert), pinnée client |
| **Opérationnelle** | hors-ligne tiède (laptop/HSM admin) | signe les descripteurs de nœuds + l'enveloppe annuaire |
| **Serveur** | en ligne (warren-api) | signe l'enveloppe de fraîcheur (génération/expiry), comme `/v1/exits` |

**Invariant** : warren-api ne détient **jamais** la root ni
l'opérationnelle. Une compromission totale de l'API ne permet que de
servir des descripteurs réellement signés hors-ligne (au pire rejouer un
set authentique borné par anti-rollback + expiry). Identique au roster
existant (`/v1/exits/roster`, *« holds no admin key, cannot forge »*).

Chaîne de vérif client (le client **pin la root**) :
1. enveloppe (génération + `expires_at`) → signature serveur pinnée ;
2. fraîcheur : `expires_at > now` + `generation >= max_vu` (anti-rollback) ;
3. cert : `root_sig` sur `operational_pubkey` (chaîne
   `WARREN_PKI_ROOT_OPERATIONAL_V1` déjà existante) → rotation de
   l'opérationnelle sans re-pin client ;
4. chaque descripteur de nœud : `verify_*_descriptor` existants avec
   `operational_pubkey`.

---

## 3. Format wire

**Réutilisation maximale du /v1 existant, immuable.** Chaque nœud est
décrit par les **deux** descripteurs signés déjà définis, mintés
hors-ligne par l'opérationnelle pour le **même** `node_id` / `ed25519` /
`endpoint` :

- `RelayDescriptorSigned { relay_id, relay_ed25519_pubkey, endpoint, sig }`
  → utilisé quand le nœud est **entrée**.
- `ExitDescriptorSigned { exit_id, exit_ed25519_pubkey,
  exit_x25519_multihop_pubkey, endpoint, sig, dns_disabled }`
  → utilisé quand le nœud est **sortie**.

avec `relay_id == exit_id == node_id` et même clé ed25519 / endpoint. Zéro
nouveau type de descripteur, zéro changement aux `verify_*` existants.

Nouveau type d'annuaire (warren-api-types) :

```rust
struct NodeEntry {
    relay: RelayDescriptorSigned,   // le nœud comme entrée
    exit:  ExitDescriptorSigned,    // le même nœud comme sortie
    // métadonnées de sélection (pays/AS, déjà dans /v1/exits)
    country: String,
    city: String,
    weight: u64,
}

struct SignedMultiHopDirectory {
    generation: u64,
    issued_at: u64,
    expires_at: u64,
    operational_pubkey: [u8; 32],
    operational_cert_sig: [u8; 64],   // root → opérationnelle
    nodes: Vec<NodeEntry>,            // UN seul pool, tous dual-rôle
    envelope_sig: [u8; 64],           // clé serveur (fraîcheur)
}
```

> Futur /v2 possible : fusionner relay+exit en un seul `NodeDescriptor`
> signé une fois. Hors scope, on réutilise le /v1 tel quel.

### Endpoints warren-api (calqués sur le roster)

| Méthode | Route | Auth | Rôle |
|---|---|---|---|
| `GET` | `/v1/multihop/directory` | public | sert `SignedMultiHopDirectory` |
| `POST` | `/v1/admin/multihop/directory` | admin | publie l'annuaire minté hors-ligne |

---

## 4. Changements par composant (simplifié vs design précédent)

### C1, warren-exit DEVIENT un nœud dual-rôle
- **Embarque le forwarder relai** : `warren-exit` dépend de `warren-relay`
  en lib et lance aussi le chemin entrée (forward aveugle C1→C2). Le même
  serveur QUIC sert les deux rôles via l'opaque type byte du 1er datagram
  (déjà spécifié). **C'est le cœur du « tous relais par défaut ».**
- **Publie sa clé HPKE** `exit_x25519_multihop_pubkey` au heartbeat
  (`RegisterExitRequest`), liée à son identité ed25519 authentifiée.
- L'allowlist des exits vers qui forwarder = **l'annuaire lui-même**
  (chaque nœud fetch le directory), plus de TOML statique.

### C2, warren-api-types
- `RegisterExitRequest` : `+ exit_x25519_multihop_pubkey_hex: Option<String>`
  (`#[serde(default)]`, non-breaking).
- `+ NodeEntry`, `+ SignedMultiHopDirectory`.

### C3, warren-api
- `ExitRecord` : `+ exit_ed25519_pubkey`, `+ exit_x25519_multihop_pubkey`.
- `register_exit` : stocke la clé HPKE.
- `+ list_multihop_directory` (calqué `list_exit_roster`) `+
  admin_set_multihop_directory` (calqué `admin_set_roster`).
- (Enveloppe signée serveur au GET, comme `/v1/exits`.)

### C4, warren-wapi : mint hors-ligne
- `admin-publish-multihop-directory` : lit le fleet actif (exits + leurs
  clés HPKE désormais publiées), minte pour **chaque nœud** son couple
  {relay, exit} descripteurs avec la clé opérationnelle hors-ligne,
  assemble + signe l'annuaire, `POST` à l'API.

### C5, warren-multihop
- `+ verify_multihop_directory(root_pin, server_pin, body)` (enchaîne
  enveloppe → cert → descripteurs). Tests PKI : tamper enveloppe/cert/
  descripteur, expired, rollback.

### C6, daemon (warren-app) : le client
- `+ warren_multi_hop_updater.rs` (calqué `warren_relay_list_updater.rs`) :
  fetch `GET /v1/multihop/directory`, vérifie, cache atomique, anti-rollback.
- **Sélection** : tire 2 nœuds **distincts** du pool (entrée + sortie),
  honorant `settings.warren_multi_hop.{entry_country,exit_country}` ;
  entrée ≠ sortie obligatoire ; réutilise la pondération
  `warren_relay_selector`. Construit `MultiHopConfig { relay: entrée.relay,
  exit: sortie.exit, operational_pubkey, ... }`.
- `set_warren_multi_hop(Some(cfg))` (setter **déjà ajouté**) + reconnect
  si toggle ON.
- Pin **root** baké (+ override env `WARREN_MULTIHOP_ROOT_PUBKEY`).
- `warren_multi_hop.rs` (loader fichier) rétrogradé en override dev.
- **`entry_country`/`exit_country` enfin consommés.**

### ~~C7 relay séparé~~, SUPPRIMÉ
Plus d'infra relai distincte ni d'updater relai séparé. Chaque nœud =
exit + relai et fetch le même annuaire. Le fleet unifié élimine ce
composant entier.

---

## 5. Séquencement (commits atomiques, TDD)

1. **C2** types wire (non-breaking).
2. **C1-publish + C3-store** : le nœud publie sa clé HPKE → l'API stocke. Test round-trip heartbeat.
3. **C5** `verify_multihop_directory` + tests PKI (RED→GREEN).
4. **C3-serve** endpoints GET/POST (calqués roster) + enveloppe serveur.
5. **C4** `wapi admin-publish-multihop-directory` (mint hors-ligne).
6. **C6** updater + sélection 2-nœuds-distincts + assemblage `MultiHopConfig` + pin root + conso country. ← **ici cocher le toggle produit un vrai multi-hop dynamique.**
7. **C1-forward** : `warren-exit` embarque le rôle forwarder (dual-rôle effectif). Bench Hetzner : node B entrée → node D sortie, IP sortie ≠ IP entrée, `session_kind=MultiHop`.

> Étape 7 (forwarder dans l'exit) peut précéder le bench ; tant qu'elle
> n'est pas faite, le fleet a les descripteurs mais les nœuds ne
> forwardent pas encore, donc 1→6 = chaîne client complète, 7 = active
> le dual-rôle data-plane.

---

## 6. Décisions verrouillées

1. **Enveloppe de fraîcheur** : signée par la **clé serveur** (en ligne),
   descripteurs + cert toujours hors-ligne. Liveness + cohérent `/v1/exits`.
2. **Expiry annuaire** : **6 h** (aligné `hpke_epoch_rotation`).
3. **Pin root client** : **baké build-time + override env**
   `WARREN_MULTIHOP_ROOT_PUBKEY` (pattern pin serveur / roster existant).
4. **Diversité entrée/sortie** : **pays différent OBLIGATOIRE**. **AS
   différent exigé seulement si le fleet contient ≥ 2 AS**, sinon fallback
   (warning log), sans ce fallback, un fleet mono-hébergeur (ex. tout
   Hetzner) ne pourrait jamais monter de circuit. La sélection annote
   chaque nœud de son ASN (dérivé de l'IP ou champ fleet) pour appliquer
   la règle.
5. **Scope** : **chaîne client 1→6 d'abord**, puis 7 (forwarder dual-rôle
   dans `warren-exit`).

---

## 6bis. État d'implémentation (au fil de l'eau)

**Livré, testé, clippy-clean, audité :**
- **C1** `warren-exit` publie sa clé HPKE (dérivée de l'identité Ed25519, par défaut) au heartbeat.
- **C2** types wire : `RegisterExitRequest`/`RegisterExitBody` + `exit_x25519_multihop_pubkey_hex` (non-breaking, lock wire vérifié) ; `SignedMultiHopDirectory` + `NodeEntry` + `MultiHopDirectoryDraft`.
- **C3** `warren-api` : `ExitRecord` stocke la clé HPKE (préservée sur heartbeat legacy) ; `GET /v1/multihop/directory` (wrap draft + enveloppe serveur, expiry 6 h glissante) ; `POST /v1/admin/multihop/directory` (validate self-consistent + store) ; `AdminExitRow` expose la clé HPKE. 4 tests d'intégration.
- **C4** `wapi admin-publish-multihop-directory` : lit le fleet, minte les descripteurs op-signés offline, certifie l'op key avec la root, publie le draft.
- **C5** `warren-multihop::operational_cert` (cert root→op, 5 tests) + `warren-relay-selector::multihop_directory` (sign/verify chaîné, 10 tests).
- **C6** daemon : `warren_multi_hop_directory` (fetch+verify+select+assemble+updater) ; toggle piloté via watch channel ; reconnexion à chaud ; 10 tests.

**Audit sécurité (security-auditor), findings traités :**
- ✅ **C1-audit (Critical, anti-rollback)** : high-water mark `highest_generation` côté client ; un annuaire de génération inférieure est rejeté (garde le circuit courant).
- ✅ **H1 (fail-open TOFU)** : `RootPinMode` explicite. Pin absent/garbage = `Unconfigured` → multi-hop **désactivé (fail-closed)** + warn ; TOFU exige `WARREN_MULTIHOP_ROOT_PUBKEY=INSECURE_TOFU` (warn fort).
- ✅ **L2** : fallback fichier local **uniquement** sur erreur transport (Http/Status/NotPublished) ; un échec de vérif (`Verify`/`Expired`) ne rescue jamais via fichier non signé et ne vide pas le circuit courant.
- ✅ **L3** : log `warn` quand `dropped > 0` (nœud non vouché par l'op key).
- ✅ **M2** : `assemble` défensif (`.get()` → `Option`, plus de panic d'index).
- ✅ Vérif d'enveloppe : le préimage serveur couvre `operational_pubkey_hex` + `operational_cert_hex` + `nodes` + `generation` + `expires_at` → pas de swap de descripteur/cert sans casser la chaîne (confirmé par l'audit).

**Durcissement /v2, ✅ FAIT (H2 + M4) :**
- Annuaire **v2** (`MULTIHOP_DIRECTORY_VERSION = 2`) : chaque `NodeEntry`
  porte une **attestation opérationnelle** (`attestation_hex`,
  `warren_multihop::{sign,verify}_node_attestation`, contexte
  `WARREN_PKI_OPERATIONAL_NODE_V1`) liant `node_id || exit_ed25519 ||
  asn || country` sous la clé **opérationnelle hors-ligne**.
  → **H2** : la diversité pays/AS devient **cryptographique** (un
  warren-api compromis ne peut plus relabeller la géo, test
  `node_with_relabeled_geo_is_dropped`). → **M4** : l'identité Ed25519
  RPC de l'exit est signée (plus de redirection du pin TLS de sortie).
  Vérifié par `node_fully_vouched` (relay + exit + attestation) ; un
  nœud non attesté est droppé. Minté par `wapi`.

**Durcissements restants (non bloquants) :**
- **M1 (ASN réel)** : `wapi` minte `asn:0` (attesté mais inconnu) → la
  règle AS reste relâchée tant que l'enrichissement GeoIP n'est pas
  ajouté. Le pays (dimension de diversité obligatoire) est, lui,
  pleinement attesté.
- **L1** : `wapi` lit la géo du fleet via l'API admin en ligne ; idéal =
  manifeste offline / diff de confirmation opérateur (le pays signé n'est
  plus modifiable post-mint, mais l'opérateur signe ce que l'API lui
  présente).
- **Refresh périodique** de l'annuaire côté relai dual-rôle (fetch au
  boot pour l'instant).

## 7bis. C7, forwarder dual-rôle (data-plane) : ✅ IMPLÉMENTÉ

Modèle retenu (le plus sûr, zéro modif du data-plane qui marche) : le nœud
`warren-exit` fait tourner **en plus** le forwarder `warren-relay` (déjà
complet et testé) **en second listener in-process**, sur une adresse
distincte. Le descripteur relai de l'annuaire annonce cette adresse comme
endpoint d'entrée ; le descripteur exit annonce l'adresse de terminaison.

Contrainte de cycle résolue : `warren-relay` dépend de `warren-tunnel`
(pas l'inverse), donc la composition vit dans `warren-exit` (qui dépend
des deux), `warren-tunnel` n'est pas touché.

Livré :
- `warren-exit/src/dual_role.rs::run_relay_forwarder`, boucle d'accept
  réutilisant verbatim `warren-relay` (`RelayServer`, `ExitConnPool`,
  `extract_dispatched_exit`, `forward_session`, `RelayMetrics`). Smoke
  test (bind + shutdown propre).
- Pool d'exits **sourcé de l'annuaire signé vérifié** (chaque
  `ExitDescriptorSigned` est op-vérifié avant tout dial), un warren-api
  compromis ne peut pas pointer le forwarder vers un exit rogue.
- `warren-api-client::get_multihop_directory` (GET public).
- Flag `warren-exit --dual-role-relay-listen <addr>` + `spawn_dual_role_relay`
  (fetch → vérifie pins serveur+root → construit le pool → spawn).

La correction du forwarding est couverte par les tests e2e de
`warren-relay` (`relay_e2e_local.rs`, fonctions identiques) + le smoke
test du wrapper. **Validation restante = bench multi-nœuds réel** (2
nœuds Hetzner : client → nœud B entrée → nœud D sortie, IP de sortie ≠
IP d'entrée), infra, pas code.

Caveats : refresh périodique de l'annuaire côté relai (fetch au boot pour
l'instant) + convention d'endpoint relai (adresse dédiée à annoncer dans
le descripteur relai au mint) = finitions ops documentées.

## 7. Critère « livré »

Cocher le toggle → le daemon fetch l'annuaire signé, le vérifie
(root→opérationnelle→descripteurs + fraîcheur), tire **2 nœuds distincts**
du fleet, assemble `MultiHopConfig`, reconnecte ; le trafic entre par le
node B et **sort par le node D** (egress ≠ IP d'entrée), **sans aucun
fichier manuel**, clé opérationnelle **jamais en ligne**. Vérifiable :
`warren status` montre entrée+sortie distinctes, IP publique = node D ≠
node B, logs `session_kind=MultiHop`.
