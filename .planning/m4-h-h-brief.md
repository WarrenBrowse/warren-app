# Phase M4.H.H - Site web warrenbrowse.com (marketing + acquisition)

> Brief d'agent autonome. Scope WEB séparé (repo dédié, parallélisable
> avec M4.H.G qui touche warren-app + warren-core).
> Doctrine §0.0 INVIOLABLE destructive git + §0.5 full autonomy.
> La commande `/goal` compagne pointe vers ce fichier.

**Effort estimé** : wall-clock 3-5 jours.
**Coût Hetzner** : 0 EUR (hosting Cloudflare Pages / Vercel / GitHub
Pages = gratuit pour ce volume).
**Pré-condition** :
- `gh` CLI configuré avec accès org `WarrenBrowse` (user `poka-IT`)
- Domain `warrenbrowse.com` enregistré chez Regery.com + DNS Google
  Domains (gérable par poka)
- Repo `WarrenBrowse/warrenbrowse-site` à créer (n'existe pas encore)

**Objectif** : créer le site web marketing/acquisition
`warrenbrowse.com` qui communique le pitch produit Warren + assure
le download des installers Warren signed (post-M4.H.D). 6+ pages
fondamentales, design moderne dark-mode default privacy audience,
i18n FR + EN, déploy auto via push main.

---

## 0.0 INVIOLABLE - pas de commande git destructive

Quelle que soit la situation (test, recovery, "voir si ça compile",
diagnostic, expérimentation), tu ne dois JAMAIS exécuter :

- `git stash` (et toutes variantes)
- `git checkout <path>` ou `git checkout -- .`
- `git restore <path>` ou `git restore .`
- `git reset --hard <ref>`
- `git reset --hard` (sans ref)
- `git clean -fd` (et toutes variantes destructives)

Cette interdiction PRIME sur le mandat d'autonomie §0.5. ESCALADE
poka via AskUserQuestion AVANT toute commande destructive
hypothétique. Pour tester un état antérieur : `git show <ref>:<path>`
(read-only). Violation = scope error CRITIQUE.

Incident M4.H.F 2026-05-20 : agent a perdu 5 fichiers WIP poka
warren-core. Pour traçabilité, voir memory warren-app
`feedback_no_destructive_git_in_agent_briefs`.

---

## 0. MANDAT STRICT

Tests pertinents (pour les pages avec logique JS), pas em-dash,
anglais comments code/markdown, conventional commits subject-only.

---

## 0.5 MANDAT D'AUTONOMIE

Plein mandat pour atteindre le verdict GO. Diagnostic 30 min → fix
tactique → commit + push → reprise.

Escalade `AskUserQuestion` SEULEMENT si :
1. Secret leak
2. Coût hosting > 0.30 EUR (improbable pour static site)
3. Décision business juridique sensible (mentions légales, structures
   holcommOn SAS + warrenBrowse SRL, capital, juridiction)
4. **Spécifique M4.H.H** :
   - Branding assets (logo Warren, palette couleurs, typographie)
     manquants → demander à poka pointers vers assets warren-app
     desktop ou tu produis un design from scratch tactique
   - DNS update warrenbrowse.com vers déploiement → poka fait la
     mise à jour, agent fournit les targets DNS (CNAME, A records)
   - Décision juridique pricing affiché (7 EUR vs 10 EUR vs offers
     plans) → poka tranche

Décisions tactiques agent autorisées :
- Stack web (recommandation : Astro static + Tailwind + Markdown
  content)
- Hosting plateforme (recommandation : Cloudflare Pages, gratuit +
  CDN global + privacy-friendly audience)
- Structure repo (monorepo single Astro project vs separation)
- Schéma URLs (/fr/pricing vs /pricing?lang=fr vs subdomain)
- Component library (headless via Radix, ou Astro components purs)
- Color palette si pas fournie par poka (recommandation : dark
  mode default, accent color cohérent warren-app desktop)
- Sources content pour comparatif 5 concurrents (cf. memory
  `feedback_warren_competitor_comparatives` warren-core)

---

## 1. Optimisations agent

- Lectures sources orchestration en PARALLÈLE en début de phase
- Commit + push au fil de l'eau (10+ commits attendus pour un site
  de 6+ pages)
- Test deploy intermédiaire dès page 2 livrée (validate end-to-end
  pipeline)

---

## 2. Setup initial

```bash
# Aucun repo warren-app n'a besoin d'être touché par cette phase.
# L'agent travaille principalement dans un nouveau repo
# WarrenBrowse/warrenbrowse-site cloné dans :
mkdir -p /Users/poka/dev/warrenBros/warrenbrowse-site
cd /Users/poka/dev/warrenBros/warrenbrowse-site

# Vérifier gh CLI accès poka-IT à l'org WarrenBrowse
gh auth status
gh repo list WarrenBrowse --limit 5

# Si poka-IT pas actif : gh auth switch --user poka-IT
```

---

## 3. Sources à lire (PARALLÈLE)

### Pitch produit Warren (NÉCESSAIRE pour le content)

- Memory `warren_product_corrected.md` warren-core (business model,
  structure juridique holcommOn SAS + warrenBrowse SRL, ticket
  7-10 EUR/mois, concurrents 5, **PAS afficher** chiffres internes
  CA Y2 / capital / marketing budget)
- Memory `warren_multihop_doctrine_v1.md` warren-core (architecture
  Apple Private Relay + HPKE multi-hop)
- Memory `warren_obfuscation_doctrine_v1.md` warren-core (M4.0 HTTP/3
  mimicry bidirectionnelle)
- Memory `feedback_warren_competitor_comparatives.md` warren-core
  (5 concurrents OBLIGATOIRES Mullvad + ProtonVPN + AirVPN + IVPN
  + Obscura, sources fiables, pas de cherry-pick, pas de chiffres
  confabulés)
- Memory `warren_m4e_delivered.md` warren-core (perf cross-DC 409 Mbps
  sustained 30 min validée empiriquement = chiffres communicables)
- Memory `warren_m4h_a_quart_delivered.md` warren-app (802 Mbps
  single-hop sustained 5 min = chiffres communicables)
- Memory `warren_m4h_f_delivered.md` warren-app (NAT-PMP port-
  forwarding différenciateur vs Mullvad/IVPN abandon 2023)

### Existing assets warren-app (branding source)

- `desktop/packages/mullvad-vpn/assets/images/` (logo + icons
  graphiques warren post-rebrand R1)
- `dist-assets/icon-macos.icns` + `icon.icns` + `icon.ico` (icons
  applicatifs warren)
- `graphics/` warren-app si existant

### Memory cross-session warren-app

- `feedback_no_destructive_git_in_agent_briefs.md` CRITIQUE
- `feedback_agent_full_autonomy_no_timid_rollback.md`
- `feedback_warren_no_secrets_in_commits.md` warren-core
- `feedback_no_em_dash.md` warren-core

---

## 4. Plan d'exécution

### M4.H.H.0 - Bootstrap repo + stack

1. `gh auth switch --user poka-IT` puis verify access.
2. `gh repo create WarrenBrowse/warrenbrowse-site --public --description "Warren VPN — Privacy-first, full-QUIC, port-forwarding restored" --license AGPL-3.0` (ou MIT selon préférence agent, AGPL-3.0 recommandé pour cohérence avec ethos privacy/open warren).
3. **NB : public** car site marketing. Vs warren-app private POC.
4. Clone local `/Users/poka/dev/warrenBros/warrenbrowse-site/`.
5. Initialize Astro project :
   ```bash
   npm create astro@latest . -- --template minimal --typescript strict --no-install
   npm install
   npx astro add tailwind
   npx astro add sitemap
   npx astro add mdx
   ```
6. Configure `astro.config.mjs` : i18n FR + EN, base URL,
   `site: 'https://warrenbrowse.com'`.
7. Commit `chore(bootstrap): astro 5 + tailwind + i18n FR/EN setup`.
8. Push origin/main → trigger initial Cloudflare Pages deploy (à
   setup §M4.H.H.5).

### M4.H.H.1 - Landing page + navigation

1. Créer `src/pages/index.astro` (EN default) + `src/pages/fr/index.astro`.
2. Sections landing :
   - **Hero** : "Privacy-first VPN, restored port-forwarding, French
     trust" (FR : "Le VPN privacy-first qui n'a rien abandonné")
   - **3 différenciateurs en haut** :
     - 🛡️ Full-QUIC pur Rust (vs WireGuard chez Mullvad/Proton/IVPN)
     - 🔀 Multi-hop pattern Apple Private Relay (HPKE bidirectionnel
       end-to-end)
     - 🔌 Port forwarding restauré (vs Mullvad/IVPN abandon 2023)
   - **Trust signals** : holcommOn SAS (France) + warrenBrowse SRL
     (Romania), audit policy, GPL-3.0 daemon source
   - **CTA** primary : Download Warren VPN
   - **CTA** secondary : See pricing
3. Header navigation : Features / Pricing / Compare / Download / FAQ
4. Footer : Privacy / Terms / Legal / Open source code (warren-core +
   warren-app GitHub links)
5. Tests Vitest pour les utility components si applicable.
6. Commit `feat(pages): landing FR + EN with hero + differentiators + trust signals`.

### M4.H.H.2 - Features page (5 différenciateurs détaillés)

1. `src/pages/features.astro` + `src/pages/fr/features.astro`.
2. Sections détaillées :
   - **Full-QUIC pur Rust** : explication stack Quinn + GSO patch +
     perf 802 Mbps single-hop / 409 Mbps multi-hop sustained mesurés
     cross-DC
   - **Multi-hop Apple Private Relay pattern** : two-relayed QUIC +
     HPKE bidirectionnel (RFC 9180) + différence vs WireGuard chained
   - **Obfuscation HTTP/3 mimicry** : ALPN h3 + SNI .exits.warrenbrowse.com
     + Initial split + port 443 + spin bit random, défait GFW SNI
     extractor April 2024 (USENIX'25)
   - **Port forwarding restauré** : NAT-PMP RFC 6886, lifetime
     auto-renewal, badge "Unique vs major competitors who abandoned
     this feature in 2023"
   - **Auth wallet Ed25519 non-custodial** : pas de email/account
     number, BIP39 mnemonic backup/restore, pubkey identifier
3. Pour chaque feature : icon + 2-3 paragraphes + tech link (RFC
   ref + GitHub source).
4. Commit `feat(pages): features detailed with 5 differentiators`.

### M4.H.H.3 - Pricing page

1. `src/pages/pricing.astro` + `/fr/pricing.astro`.
2. Affichage pricing :
   - 1 month : €X (à confirmer poka)
   - 6 months : €X (souvent -10-15%)
   - 1 year : €X (souvent -20-30%)
   - Anonymous payment options : crypto (BTC/Monero), cash
     (par courrier comme Mullvad)
3. **Escalade poka** sur pricing exact + options paiement supportées
   (cash, crypto, card, etc.).
4. CTA "Start subscription" → flow signup (à designer plus tard ou
   placeholder mailto)
5. FAQ pricing : refund policy, billing cycle, anonymous payment.
6. Commit `feat(pages): pricing FR + EN (placeholders pour montants
   exacts à valider poka)`.

### M4.H.H.4 - Comparison page (5 concurrents)

1. `src/pages/compare.astro` + `/fr/compare.astro`.
2. Tableau comparatif OBLIGATOIRE 5 concurrents (cf. memory
   `feedback_warren_competitor_comparatives`) :
   - Mullvad
   - ProtonVPN
   - AirVPN
   - IVPN
   - Obscura
   - + Warren
3. Lignes comparées :
   - Transport (QUIC vs WireGuard vs OpenVPN)
   - Multi-hop architecture (HPKE Apple Private Relay vs WG-chained
     vs OpenVPN cascade vs WG-over-QUIC)
   - Obfuscation (HTTP/3 bidirectionnel mimicry vs none vs Shadowsocks
     vs autres)
   - Port forwarding (restored vs abandoned 2023 vs available)
   - Auth (Ed25519 wallet vs email vs token vs account number)
   - Jurisdiction (FR+RO vs SE vs CH vs IT vs GI vs US)
   - Open source daemon (GPL-3.0 vs vs vs)
   - Audit (annual external)
4. **Pas de cherry-pick, pas de confabulation**. Si chiffre pas
   disponible : "Not publicly available" plutôt que skip silencieux.
5. Sources liées en footnote.
6. Commit `feat(pages): comparison vs 5 main VPN competitors with sources`.

### M4.H.H.5 - Download page + hosting setup

1. `src/pages/download.astro` + `/fr/download.astro`.
2. Sections :
   - macOS : Warren VPN.dmg (link à GitHub Release artifact post-
     M4.H.D ship)
   - Linux : .deb + .rpm + AppImage
   - Windows : .msi
   - Source code : link `github.com/WarrenBrowse/warren-app` + `warren-core`
   - Verify checksum : SHA-256 listé par release
3. **Placeholder pour le moment** : "Coming soon - beta release in
   progress" car premier release.yml triggered nécessite caveats ops
   poka résolus (signing assets + GH Actions billing).
4. **Hosting setup Cloudflare Pages** :
   - Connect GitHub `WarrenBrowse/warrenbrowse-site` → Cloudflare Pages
   - Build command : `npm run build`
   - Output dir : `dist`
   - Custom domain : `warrenbrowse.com` (escalade poka pour ajouter
     CNAME ou nameserver delegation)
   - Production deploy auto sur push main, preview deploy sur PR
5. **Escalade poka** : fournir DNS records targets pour
   warrenbrowse.com (Cloudflare Pages donne `<project>.pages.dev`,
   poka configure CNAME / A records).
6. Commit `feat(pages): download placeholder + Cloudflare Pages config`.

### M4.H.H.6 - FAQ + Security + Privacy + Terms + Legal

1. `src/pages/faq.astro` + `/fr/faq.astro` : 10-15 questions
   communes.
2. `src/pages/security.astro` + `/fr/security.astro` : architecture
   HPKE + Quinn + no-log policy + threat model.
3. `src/pages/privacy.astro` + `/fr/privacy.astro` : politique no-log
   stricte, GDPR compliance, data retention 0.
4. `src/pages/terms.astro` + `/fr/terms.astro` : conditions d'utilisation,
   refund policy, acceptable use.
5. `src/pages/legal.astro` + `/fr/legal.astro` : mentions légales
   holcommOn SAS (FR holding, capital 21k EUR) + warrenBrowse SRL
   (RO opérationnel, capital 20.4k EUR) + RGPD DPO contact.
6. **Escalade poka** pour content légal sensible : adresses sièges
   sociaux, RCS numbers, numéro TVA, DPO email, etc.
7. Commits séparés par page (5 commits) ou groupés selon scope (1
   commit "legal/privacy/terms section").

### M4.H.H.7 - Validation + deploy

1. `npm run build` PASS (no broken links, no missing translations).
2. `npm run lint` PASS si linter configuré.
3. Lighthouse score visé : >90 performance + 100 accessibility + 100
   best-practices + 100 SEO.
4. Manual smoke : preview deploy Cloudflare Pages accessible via
   `<project>.pages.dev` URL.
5. Si domain DNS updated : `warrenbrowse.com` résout + HTTPS valid.

### M4.H.H.8 - Finalize + commits + memory

1. Rapport `/tmp/m4-h-h-report.md` ≤ 150 lignes.
2. 10+ commits atomiques poussés origin/main (
   `github.com/WarrenBrowse/warrenbrowse-site`).
3. Memory `warren_m4h_h_delivered.md` warren-app + index MEMORY.md.
4. Memory `warren_site_repo.md` warren-app (cross-ref : repo
   warrenbrowse-site existe, hosting Cloudflare Pages, deploy
   pipeline).

---

## 5. Règles non-négociables

### Sécurité

- Pas de secrets verbatim (analytics keys, API tokens) en commit
- Pas de log user IP / fingerprinting côté site (privacy-first VPN
  site doit respecter sa propre doctrine)
- HTTPS strict, HSTS, CSP strict
- Pas d'analytics tiers privacy-hostile (PAS Google Analytics, PAS
  Facebook Pixel). Si analytics : Plausible / Umami auto-hosté ou
  Cloudflare Web Analytics aggregated.

### Code

- TypeScript strict pour scripts
- Pas em-dash, anglais comments
- Conventional commits subject-only, pas Co-Authored-By Claude
- Accessibility WCAG AA minimum

### Git

- Push main direct warrenbrowse-site (nouveau repo, pas de feature
  branch needed)
- §0.0 INVIOLABLE rappelé en tête

### Content

- Pas de cherry-pick chiffres concurrents (cf. memory
  `feedback_warren_competitor_comparatives`)
- Pas de confabulation chiffres perf (utiliser uniquement chiffres
  warren-core mesurés : 409 Mbps multi-hop, 802 Mbps single-hop, 30
  min sustained)
- Pas de claim "Cure53 audit" (cf. memory `warren_no_cure53_audit`)
- Pas de mention chiffres internes business (capital, CA cible,
  marketing budget) - tous PRIVÉS

---

## 6. Pas de validation intermédiaire poka

§0.5. 4 cas escalade + 3 cas M4.H.H spécifiques.

---

## 7. Critères phase livrée

### GO ULTIMATE (cible)

- Repo `github.com/WarrenBrowse/warrenbrowse-site` créé public AGPL-3.0
- Stack Astro 5 + Tailwind + i18n FR/EN + Markdown content
- 10+ pages livrées (index + features + pricing + compare + download
  + FAQ + security + privacy + terms + legal en FR + EN = ~20 pages
  total)
- Tableau comparatif 5 concurrents avec sources
- 0 chiffre confabulé, 0 mention Cure53, 0 leak chiffres internes
- Cloudflare Pages deploy fonctionne (preview URL accessible)
- Lighthouse score >90/100/100/100
- DNS warrenbrowse.com configurable (poka mise à jour cf. escalade)
- 10+ commits atomiques poussés origin/main warrenbrowse-site
- Memory updates warren-app

### GO CONDITIONAL

- 6/10 pages livrées (core : index + features + pricing + compare
  + download + FAQ), placeholders pour les autres
- Deploy preview OK mais DNS pas configuré (escalade poka pending)

### NO-GO HONNÊTE (improbable §0.5)

- poka-IT permissions org WarrenBrowse refusées pour creating
  warrenbrowse-site repo
- Branding assets non disponibles ET escalade poka pas répondue
  4h+

---

## 8. Rapport final attendu

`/tmp/m4-h-h-report.md` ≤ 150 lignes :

1. **Verdict** GO ULTIMATE / CONDITIONAL / NO-GO + 1 phrase
2. **Repo** : URL + visibility (public AGPL-3.0)
3. **Stack** : versions exactes
4. **Pages livrées** : list 10+ avec URL preview Cloudflare Pages
5. **Comparatif 5 concurrents** : sources utilisées
6. **Deploy pipeline** : Cloudflare Pages config, preview URL
7. **DNS** : status (configuré ou pending poka)
8. **Lighthouse score** : 4 métriques
9. **Escalades poka pendantes** : pricing exact, content légal,
   branding assets, DNS update
10. **Commits** + memory updates

---

## 9. Next steps post-phase (orchestrateur)

- **GO ULTIMATE** → scope dev warren-app M4.H pure CLOSED
  (M4.H.A → M4.H.H livrés). Reste pour ship beta Warren :
  - **Caveats ops poka** : GH Actions billing + WARREN_CORE_RO_TOKEN
    + signing assets + SSH Hetzner
  - **First release.yml triggered** post-caveats résolus
  - **Bench installer empirique** post-SSH résolu
- **CONDITIONAL** → pondérer pages restantes
- **NO-GO** → analyse cause root before next

Caveats persistants post-M4.H.H :
- Pricing exact + paiement options : escalade business poka
- Content légal mentions : escalade business poka
- Branding assets cohérents avec warren-app desktop : escalade poka

Phases futures éventuelles :
- M4.H.I : signup flow (forme + back-end + crypto payment)
- M4.H.J : status page (uptime monitoring) si pertinent

---

## 10. Trace de mémorisation

Warren-app memory dir (orchestrateur acte la nouvelle prop) :
- Create `warren_m4h_h_delivered.md`
- Index MEMORY.md : `- [M4.H.H delivered](warren_m4h_h_delivered.md) — <verdict> warrenbrowse.com Astro static site 10+ pages FR+EN comparatif 5 concurrents + Cloudflare Pages deploy`
- Optional reference memory : `warren_site_repo.md` pour cross-ref
  vers le nouveau repo warrenbrowse-site

Warrenbrowse-site repo (nouveau) :
- README.md auto-généré par Astro init, à enrichir pour devs
  contributeurs
- Pas de memory cross-session warren-* dans ce nouveau repo (scope
  séparé)
