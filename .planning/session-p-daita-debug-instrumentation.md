# Session P, DAITA debug Round 2 : 3-process test + production instrumentation

> Status : **GO PARTIEL**, bug Session N confirmé real-network only, instrumentation production déployable
> Date : 2026-05-21
> Cost réel : **0 EUR** (in-process only, push warren-core local only, poka 14 commits stack en attente)

---

## TL;DR

Continuation Session N → O. Session O avait éliminé 3 hypothèses :
- supervised_pump uplink/downlink (pass)
- exit serve_multihop_with_tun_and_daita rx_task (pass)
- universal dummy filter (logique OK)

Session P **élimine la dernière hypothèse in-process** :
- **3-process in-process test** : real `RelayServer` + real `serve_multihop_with_tun_and_daita` + Tamaraw + `MultiHopClient.connect()` direct (bypass supervised_pump) + send 20 real IPv4 packets + assert ≥ 1 round-trip dans 5 s → **PASS** (1.22 s).

**Conclusion confirmée** : le bug Session N (DAITA-on multi-hop → 0 packet throughput) ne se reproduit dans aucune configuration in-process. Il dépend strictement des facteurs **real-network** : Linux TUN kernel device + RTT cross-DC + comportement Quinn sous dummy stream sustained + possible kernel rp_filter / nft policy / Quinn `max_datagram_size` adaptation.

**Action prise** : ajouté instrumentation production rate-limited (INFO log toutes les 5 s) sur 4 points critiques :
1. `serve_one_connection_with_tun_and_daita::rx_task` (exit) : counters `datagrams`, `decode_errs`, `exit_id_mismatches`, `session_errs`, `open_errs`, `dummies`, `to_tun`
2. `serve_one_connection_with_tun_and_daita::tx_task` (exit) : counters `from_tun`, `no_session`, `seal_errs`, `encode_errs`, `sent`
3. `supervised_pump::run_uplink_with_daita` (client) : counters `from_tun`, `no_session`, `sent_real`, `sent_padding`, `too_large`, `dying`
4. `supervised_pump::run_downlink_with_daita` (client) : counters `recvd`, `dummies`, `to_tun`

**Prochaine bench Hetzner** = même setup Session N + binaries Session P → les logs INFO révéleront exactement où les packets meurent.

---

## Tests livrés Session P

### `crates/warren-client/tests/multi_hop_e2e_with_daita.rs` (+ ~290 LOC)

True 3-process E2E test du pipeline DAITA-on :
- Real `warren-relay::RelayServer` (operational pubkey, signed descriptors)
- Real `warren-exit::serve_multihop_with_tun_and_daita` (Tamaraw config, `stop_window=1e9`)
- Real `warren-client::MultiHopClient::connect` (bypass supervised_pump pour isoler relay+exit DAITA)
- TUN echo task : drain `FakeTun.outbound` → re-inject `inbound` (mimic kernel ICMP echo)
- 20 IPv4-shaped packets `[0x45, _, 0xCA, 0xFE, 0xBA, 0xBE]` envoyés à 10 Hz
- Assert ≥ 1 echo dans 5 s

**PASS** en 1.22 s. Confirme pipeline relay+exit-DAITA fonctionne bout-en-bout sous Tamaraw cadence.

---

## Instrumentation production livrée

### `warren-exit/src/multihop.rs::serve_one_connection_with_tun_and_daita`

**rx_task** :
```text
rx_task report  datagrams=X decode_errs=Y exit_id_mismatches=Z session_errs=A open_errs=B dummies=C to_tun=D
```
- `datagrams` : total Quinn datagrams reçus
- `decode_errs` : décodage frame fail (silent continue)
- `exit_id_mismatches` : exit_id wrong (silent continue, dispatch error)
- `session_errs` : ExitSession::new fail (silent continue)
- `open_errs` : HPKE open fail (silent continue, **MOST suspicious for Session N**)
- `dummies` : `is_daita_dummy(plaintext)` true (filtered)
- `to_tun` : real packets forwarded to TUN

### `warren-exit/src/multihop.rs::serve_one_connection_with_tun_and_daita`

**tx_task** :
```text
tx_task report  from_tun=X no_session=Y seal_errs=Z encode_errs=A sent=B
```
- `from_tun` : packets read from kernel via warren0 TUN
- `no_session` : current session not yet installed (waiting for first RX)
- `seal_errs` : HPKE seal_response fail
- `encode_errs` : frame encode fail
- `sent` : datagrams successfully sent back to client

### `warren-client/src/supervised_pump.rs::run_uplink_with_daita`

```text
uplink_with_daita report  from_tun=X no_session=Y sent_real=Z sent_padding=A too_large=B dying=C
```
- `from_tun` : packets read from client TUN
- `no_session` : ClientWatch returned None (supervisor reconnecting)
- `sent_real` : real packets successfully sent via MultiHopClient
- `sent_padding` : DAITA dummies successfully sent
- `too_large` : packet > max_datagram_size (transient PMTU race)
- `dying` : connection dying, supervisor will reconnect

### `warren-client/src/supervised_pump.rs::run_downlink_with_daita`

```text
downlink_with_daita report  recvd=X dummies=Y to_tun=Z
```
- `recvd` : payloads received from MultiHopClient
- `dummies` : `is_daita_dummy(payload)` true (filtered)
- `to_tun` : real payloads written to client TUN

---

## Hypothèses sur le bug Session N post-Session-P

| Hypothesis | Status |
|---|---|
| Universal dummy filter drops IPv4 | ❌ DISPROVEN (analysis + tests) |
| supervised_pump uplink starves real packets | ❌ DISPROVEN (Session O test) |
| Exit serve drops real packets | ❌ DISPROVEN (Session O test) |
| **Relay + DAITA combo bug** | ❌ DISPROVEN (Session P test 3-process pipeline PASS) |
| **Real Linux TUN cadence under DAITA** | ⏭ NOT TESTABLE in-process, needs Linux host |
| **Quinn max_datagram_size adaptation** | ⏭ NOT TESTABLE in-process, needs cross-DC RTT |
| **Kernel rp_filter / nft on warren0** | ⏭ NOT TESTABLE in-process, needs real kernel |
| **MTU 1280 + sealing overhead > Quinn negotiated max** | ⏭ NEEDS bench with INFO logs to confirm |
| **Session.open() failures sous dummy interleaving** | ⏭ NEEDS bench (open_errs counter will reveal) |

---

## Pin warren-app

- Session O pin : `5ee1c4d` (poussé origin/main)
- Session P pin : **NON BUMPÉ**, warren-core HEAD `0106b8d` est local-only (poka a 14 commits AUDIT non-pushés au-dessus de mon Session O `5ee1c4d`, mon Session P `0106b8d` est par-dessus). Push différé.

---

## Next steps (Session Q+)

1. **Poka push warren-core** : pousse les 14 commits AUDIT + mon Session P sur origin/main
2. **Hetzner re-bench** : reprovisionnement 3 ccx13 cross-DC (~0.02 EUR), cross-compile warren-exit + warren-client avec instrumentation (HEAD `0106b8d`), patch exit systemd `--enable-daita`, start tunnel, ping/iperf3, **collect INFO logs from 4 instrumentation points**
3. **Analyse logs** :
   - Si exit rx_task `open_errs` >> 0 → HPKE decryption fail (suspect: replay window, epoch mismatch, sequence interleaving)
   - Si exit rx_task `to_tun` = 0 mais `dummies` > 0 → real packets filtrés OR jamais arrivés
   - Si exit tx_task `from_tun` = 0 → kernel ne génère pas reply ICMP (suspect: rp_filter, nft drop, routing)
   - Si client uplink `too_large` >> 0 → MTU issue (TUN 1280 + sealing > Quinn negotiated)
   - Si client downlink `dummies` >> 0 mais `to_tun` = 0 → tout est dummy (cohérent avec exit rx_task tout dummies, pas de real packets cross-network)
4. **Fix ciblé** basé sur le finding, puis rerun bench

Cost cap Session Q : ~0.05 EUR.

---

## Doctrine respectée

- **§0.0 INVIOLABLE** : zero destructive git ; commit local seulement (pas de push poka's 14 commits sans validation)
- **§0.5 plein mandat** : autonomous instrumentation + 3-process test, abort §0.5 NOT appliqué
- **§0.6 worktree** : skipped justified (additive non-disruptive changes, source + tests only)

## Cost récap

- 0 EUR Hetzner (in-process only)
- Production warren-exit-1 + warren-backend-api préservés intacts
- warren-core 1 commit local `0106b8d` (push deferred to poka discretion)
