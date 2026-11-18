//! Daemon start-up: what `Daemon::start` assembles, from the settings and the
//! account store to the tunnel state machine and the background refreshers,
//! before the event loop runs.

use crate::*;

impl Daemon {
    pub async fn start(
        config: DaemonConfig,
        daemon_command_channel: DaemonCommandChannel,
    ) -> Result<Self, Error> {
        #[cfg(target_os = "macos")]
        macos::bump_filehandle_limit();

        // F7 fork audit: `migrate_all` may fail for non-fatal reasons
        // (e.g. account-history.json absent on fresh boot Warren mode). The
        // daemon continues with `None` migration data - this is expected, not
        // an operational error. Logging WARN avoids false alarms
        // in prod logs without masking a real migration problem
        // on an existing upstream install.
        let migration_data = migrations::migrate_all(&config.cache_dir, &config.settings_dir)
            .await
            .unwrap_or_else(|error| {
                log::warn!(
                    "{}",
                    error.display_chain_with_msg("Failed to migrate settings or cache (non-fatal)")
                );
                None
            });

        let mut settings = SettingsPersister::load(&config.settings_dir).await;

        // Initialize relay selector asap, since it's a pre-requisite for accepting incoming gRPC
        // connections.
        //
        // F5 fork audit: the upstream Mullvad `relays.json` list is
        // never consumed - the tunnel
        // uses `warren-relays.json` parsed by `DaemonWarrenRelaySelector`.
        // An absence of the file should not log ERROR (= noise that
        // worries the prod operator) but just DEBUG, since the list is
        // always unused on the Warren fork.
        let initial_relay_list = parse_relays_from_file(&config.cache_dir, &config.resource_dir)
            .inspect_err(|err| {
                log::debug!("relays.json unavailable (Warren tunnel active, list unused): {err}");
            })
            .ok();
        let relay_selector = {
            let (initial_relay_list, initial_bridge_list) = initial_relay_list
                .clone()
                .map(CachedRelayList::into_internal_repr)
                .unwrap_or_default();
            // TODO: This should preferably be done once, by the relay list updater.
            let initial_relay_list =
                initial_relay_list.apply_overrides(settings.relay_overrides.clone());
            RelaySelector::from_settings(
                &settings,
                initial_relay_list.clone(),
                initial_bridge_list.clone(),
            )
        };

        // The identity manager is the single owner of both
        // identity views (signer + SDK seed). It loads the persisted
        // mnemonic if the user has onboarded, and otherwise runs on the
        // placeholder sentinel: no identity is ever minted outside the
        // explicit create/import onboarding actions. It stays in the
        // `Daemon` struct so create/restore/logout hot-swap the identity
        // without requiring a daemon restart. Loaded before the management
        // interface starts, because whether a wallet is installed is part of
        // deciding who may call it.
        let warren_identity = Arc::new(warren_identity_manager::WarrenIdentityManager::load(
            &config.settings_dir,
        ));
        if !warren_identity.is_coherent() {
            // Unreachable by construction (the manager swaps both views
            // from one seed); kept as a loud tripwire because a signer /
            // SDK-seed divergence silently strands paid subscriptions on
            // a wallet the user does not own.
            log::error!("Warren identity views diverged at boot; identity handling is broken");
        }
        let command_sender = daemon_command_channel.sender();
        let app_upgrade_broadcast = tokio::sync::broadcast::channel(32).0;
        let warren_status_cache = warren_status::WarrenStatusCache::new();
        warren_token_provider::report_clock_skew_to(warren_status_cache.clone());
        warren_token_provider::keep_rotation_in(config.cache_dir.clone());
        warren_token_provider::keep_device_in(&config.cache_dir);
        let warren_network_stats = warren_network_stats::WarrenNetworkStats::new();
        let management_interface = ManagementInterfaceServer::start(
            command_sender,
            config.rpc_socket_path,
            app_upgrade_broadcast.clone(),
            config.log_handle,
            relay_selector.clone(),
            warren_status_cache.clone(),
            warren_network_stats.clone(),
            &config.settings_dir,
            warren_identity.clone(),
        )
        .map_err(Error::ManagementInterfaceError)?;

        let (internal_event_tx, internal_event_rx) = daemon_command_channel.destructure();
        let revocation_tx = internal_event_tx.clone();
        warren_token_provider::report_revocation_to(Box::new(move |verified| {
            let _ = revocation_tx.send(InternalDaemonEvent::WarrenDeviceRevoked { verified });
        }));
        let tokens_tx = internal_event_tx.clone();
        warren_token_provider::report_tokens_to(Box::new(move || {
            let _ = tokens_tx.send(InternalDaemonEvent::WarrenSessionTokensAtHand);
        }));

        mullvad_api::proxy::ApiConnectionMode::try_delete_cache(&config.cache_dir).await;
        let api_runtime =
            mullvad_api::Runtime::with_cache(&config.endpoint, &config.cache_dir, true)
                .await
                .map_err(Error::InitRpcFactory)?;

        let api_availability = api_runtime.availability_handle();
        api_availability.suspend();

        // Every Warren fetcher resolves the API host from this same cache from
        // here on. It is the cache `api::resolve_allowed_endpoint` builds the
        // firewall's permitted endpoint from, so the address a recovery fetch
        // dials is exactly the one the blocking state lets through, and no
        // system DNS query is needed to find it.
        warren_api_dns::install(warren_api_dns::ApiHostResolver::from_address_cache(
            config.endpoint.host().to_owned(),
            api_runtime.address_cache().clone(),
        ));

        let settings_event_listener = management_interface.notifier().clone();
        settings.register_change_listener(move |settings| {
            // Notify management interface server of changes to the settings
            settings_event_listener.notify_settings(settings.to_owned());
        });

        let method_resolver = DaemonAccessMethodResolver::new(api_runtime.address_cache().clone());

        let (access_mode_handler, access_mode_provider) =
            mullvad_api::access_mode::AccessModeSelector::spawn(
                method_resolver,
                settings.api_access_methods.clone(),
                #[cfg(feature = "api-override")]
                config.endpoint.clone(),
                internal_event_tx.to_unbounded_sender(),
            )
            .await
            .map_err(Error::ApiConnectionModeError)?;

        let warren_signer = warren_identity.signer();
        // Single source of truth for the raw-`SigningKey` consumers: the
        // SAME `Arc<RwLock<SigningKey>>` that `WarrenAuthSigner` wraps is
        // shared with the tunnel (RPK identity + legacy REST signing). A
        // manager hot-swap (create/restore/logout) therefore propagates to
        // every consumer without a daemon restart, instead of leaving
        // frozen per-consumer copies signing as the old/erased identity.
        let warren_shared_key = Some(warren_signer.shared());
        // Companion shared, hot-swappable BIP39 seed for the SDK-backed
        // `warren_api::WarrenApiClient` (account backend + best-effort
        // incident reports, via `warren_sdk_client::SharedWarrenApiClient`).
        // The SDK's `WarrenIdentity` only builds from this pre-HKDF seed,
        // never from an already-derived `SigningKey` (see
        // `warren_sdk_client` module doc). Both views live inside the
        // manager, which is the only surface that ever swaps them, always
        // together and from one seed value.
        let warren_shared_seed = Some(warren_identity.seed_handle());
        let api_handle = api_runtime
            .mullvad_rest_handle_with_warren_signer(access_mode_provider, Some(warren_signer));

        // Warren uses a single fixed API host, so the upstream Mullvad
        // API-IP rotation endpoint (`GET app/v1/api-addrs`) does not exist
        // on the Warren backend and always answers 404. Spawning the fetcher
        // would just log an error every 15 minutes without ever updating the
        // cache, so we deliberately do NOT start it. The address cache stays
        // valid via its default/stored address (read in `api.rs`).

        let access_method_handle = access_mode_handler.clone();
        settings.register_change_listener(move |settings| {
            let handle = access_method_handle.clone();
            let new_access_methods = settings.api_access_methods.clone();
            tokio::spawn(async move {
                let _ = handle.update_access_methods(new_access_methods).await;
            });
        });

        let _ = migration_data;
        let migration_complete = migrations::MigrationComplete::new(true);

        // Resolution delegated to `warren_remote_config::resolve` (pure
        // testable fn). Side effects (env, signing_key load) resolved
        // here, the pure flags passed to the fn. The diff log (Some vs
        // None) stays here because the fn is silent to remain
        // testable without log capture.
        let env_url = std::env::var("WARREN_API_URL").ok();
        let warren_api_config = warren_remote_config::resolve(
            settings.warren_api_url.clone(),
            env_url,
            warren_identity.seed_handle(),
        );
        log::info!(
            "Warren remote backend enabled (api={})",
            warren_api_config.url
        );

        // The standing monitor starts further down, once the event loop's
        // plumbing exists; the account backend reports the ban a voucher
        // refusal carries into this slot, filled when it does.
        let warren_standing_slot: std::sync::Arc<
            std::sync::OnceLock<warren_account_standing::StandingMonitor>,
        > = std::sync::Arc::default();
        let warren_ban_sink: device::BanSink = {
            let slot = warren_standing_slot.clone();
            std::sync::Arc::new(move |wallet, ban| {
                if let Some(monitor) = slot.get() {
                    monitor.report_issuance_ban(wallet, ban);
                    // The unsigned refusal may omit the lapse; the signed
                    // standing answers it.
                    monitor.poke();
                }
            })
        };
        let (account_manager, data) = device::AccountManager::spawn(
            api_handle.clone(),
            &config.settings_dir,
            internal_event_tx.to_specialized_sender(),
            warren_api_config,
            Some(warren_ban_sink),
        )
        .await
        .map_err(Error::LoadAccountManager)?;

        // Boot-time identity reconciliation. The BIP39-derived signer is
        // the cryptographic root of truth: it signs every API request and
        // builds the tunnel params. `device.json` is only a cache of the
        // logged-in pubkey and can drift from the signer (e.g. the
        // secret-storage backend changed under the daemon, or a partial
        // restore). When they disagree, the displayed account and the
        // actually-signing identity diverge, which strands paid
        // subscriptions on a wallet the user does not see. Re-align
        // `device.json` to the signer here so the three views (signer,
        // device.json, `account get`) can never silently differ. Only the
        // LoggedIn case is touched: `pubkey()` is None for
        // LoggedOut/Revoked, so a deliberate logout or revocation is
        // preserved.
        if warren_identity.has_user_identity() {
            let expected = warren_identity.pubkey_ss58();
            let stored = data.pubkey().map(|p| p.as_str().to_owned());
            if let Some(target) =
                warren_signer::reconcile_login_target(&expected, stored.as_deref())
            {
                // Self-healing: WARN, not ERROR. The drift is re-aligned
                // on the spot, so it is an operational notice, not a fatal
                // condition; kept loud because an unnoticed desync strands
                // paid subscriptions.
                log::warn!(
                    "Warren identity desync at boot: signer={target} device.json={}; \
                     re-aligning device.json to the signer-derived identity",
                    stored.as_deref().unwrap_or("<none>")
                );
                let manager = account_manager.clone();
                let target = target.to_owned();
                tokio::spawn(async move {
                    match manager.login(target).await {
                        Ok(()) => log::info!(
                            "Warren identity reconciled: device.json now matches the signer"
                        ),
                        // `login` here is a local device-cache write (no
                        // network), so do NOT force a logout on failure:
                        // the only failure mode is a cache write error. The
                        // signer stays the source of truth for signing;
                        // reconciliation retries on the next boot.
                        Err(e) => log::warn!(
                            "Warren identity reconciliation failed ({e}); device.json still \
                             records a stale identity, will retry next boot"
                        ),
                    }
                });
            }
        } else if data.pubkey().is_some() {
            // device.json claims a logged-in account but no identity
            // exists on disk (secret storage lost or wiped out-of-band).
            // The daemon cannot sign as that account anymore; land the
            // user on a deterministic logged-out state instead of a
            // logged-in UI whose every request fails.
            log::warn!(
                "device.json records a logged-in account but no Warren identity is \
                 persisted; forcing logout"
            );
            let manager = account_manager.clone();
            tokio::spawn(async move {
                if let Err(e) = manager.logout().await {
                    log::warn!("boot logout after lost identity failed: {e}");
                }
            });
        }

        // The forced logout just above lands through the event loop, after the
        // boot connect decision: count it as done so the boot does not bring
        // up a tunnel for an account that is on its way out.
        let boot_login = if data.pubkey().is_some() && !warren_identity.has_user_identity() {
            warren_session_gate::Login::LoggedOut
        } else {
            warren_session_gate::Login::from(&data)
        };
        let warren_wallet_activity = warren_wallet_activity::WalletActivity::new(
            boot_login == warren_session_gate::Login::LoggedIn,
        );

        let account_history = account_history::AccountHistory::new(
            &config.settings_dir,
            // AccountHistory.set/get take an AccountNumber (= String).
            data.pubkey().map(|pubkey| pubkey.as_str().to_owned()),
        )
        .await
        .map_err(Error::LoadAccountHistory)?;

        let target_state = if settings.auto_connect {
            log::info!("Automatically connecting since auto-connect is turned on");
            PersistentTargetState::new_secured(&config.cache_dir).await
        } else {
            PersistentTargetState::new(&config.cache_dir).await
        };

        // A `Secured` state recovered from the cache means the previous
        // daemon never reached `finalize()` and so never deleted the file:
        // a host crash, a power loss, a SIGKILL. We are re-arming the
        // tunnel on the user's behalf, and this path is gated by NEITHER
        // `auto_connect` NOR the GUI's launch-at-login, which is what
        // makes it the one way to end up connected with nothing on screen
        // to say why. Surface it to every client, and log it at a level a
        // reader cannot mistake for routine: at INFO, next to the ordinary
        // "Loaded cached target state" line, it reads like bookkeeping.
        if *target_state == TargetState::Secured && target_state.restored_from_cache() {
            log::warn!(
                "Restoring the tunnel from a target state left behind by an unclean \
                 shutdown: the previous run did not exit cleanly. This is neither \
                 auto-connect nor launch-at-login, so nothing the user configured \
                 explains it"
            );
            warren_status_cache.record_restored_after_unclean_shutdown();
        }

        #[cfg(target_os = "linux")]
        let split_tunneling_pid_manager = split_tunnel::PidManager::default();

        // Init the Warren tunnel artifacts at boot. The
        // signed exit list is loaded synchronously from the **newest
        // verifying source** among the on-disk cache (last fetched) and
        // the build-time baked bootstrap in `resource_dir`, pinned to the
        // production server pubkey (anti key-swap). This never blocks on
        // the network; a background `WarrenRelayListUpdater` (spawned
        // further down) refreshes the list on startup + periodically and
        // hot-swaps it into the live selector. The BIP39 `SigningKey` is
        // loaded from the settings dir. Warren is the only tunnel mode on
        // this fork, so these artifacts are always populated.
        let warren_api_url = warren_remote_config::api_url(
            settings.warren_api_url.clone(),
            std::env::var("WARREN_API_URL").ok(),
        );
        // Public `GET /v1/network` descriptor (environment label, degraded
        // flag, default bandwidth cap, payments flag): fetched in the
        // background and pushed to both UIs through the status stream.
        warren_network_info::spawn(warren_api_url.clone(), warren_status_cache.clone());
        // Public stats snapshot: nothing is fetched until a frontend asks.
        warren_network_stats.set_api_base(warren_api_url.clone());
        // Pinned server pubkey: env override in development builds, else
        // the baked production key. Pinning is mandatory in prod.
        let warren_server_pubkey: Option<String> = Some(
            warren_product_config::anchor_override("WARREN_SERVER_PUBKEY")
                .unwrap_or_else(|| warren_product_config::WARREN_SERVER_PUBKEY_HEX.to_owned()),
        );
        // Audit F1 roster: opt-in (off by default), no pubkey baked in.
        // When enabled, the operator supplies the offline-admin pin via
        // `WARREN_ADMIN_ROSTER_PUBKEY`; an empty pin falls back to TOFU
        // (the updater warns at startup).
        let warren_roster_enabled = matches!(
            std::env::var("WARREN_ROSTER_ENABLED")
                .unwrap_or_default()
                .trim()
                .to_ascii_lowercase()
                .as_str(),
            "1" | "true" | "yes" | "on"
        );
        let warren_roster_pin: Option<String> = std::env::var("WARREN_ADMIN_ROSTER_PUBKEY")
            .ok()
            .filter(|s| !s.is_empty());
        let warren_bootstrap = warren_relay_list_updater::load_bootstrap(
            &config.cache_dir,
            &config.resource_dir,
            warren_server_pubkey.as_deref(),
        );
        // Seed the updater's anti-rollback high-water mark from the
        // bootstrap so a later fetch cannot roll back below the baked list.
        let warren_bootstrap_generation = warren_bootstrap.generation;
        // Clean up after older clients: they pinned a `<exit_ip>/32` bypass
        // route per exit, and only the exit being connected to ever gets its
        // route cleared. Left alone, a leftover turns into a black hole the
        // day the machine changes network, and it is self-locking (connecting
        // to that exit would clear it, but the route is what prevents the
        // connection). Startup is the only moment that can break the loop.
        crate::warren_stale_route_sweep::sweep_stale_exit_routes(
            &warren_bootstrap
                .relays
                .relays()
                .iter()
                .flat_map(|relay| relay.endpoint_addr().ip_addrs())
                .filter_map(|socket| match socket.ip() {
                    std::net::IpAddr::V4(addr) => Some(addr),
                    std::net::IpAddr::V6(_) => None,
                })
                .collect::<Vec<_>>(),
        );
        let (warren_relay_selector, warren_signing_key) = {
            let selector = crate::warren_relay_selector::DaemonWarrenRelaySelector::new(
                warren_bootstrap.relays,
            );
            // Share the SAME swappable key handle with the tunnel (read
            // live at connect) instead of a frozen disk re-read, so a
            // restore/create takes effect on the next connect.
            let signing_key = warren_shared_key.clone();
            if signing_key.is_none() {
                log::warn!("No Warren signing key available; tunnel attempts will fail");
            }
            (Some(selector), signing_key)
        };

        // Warren multi-hop opt-in: driven by the `warren_multi_hop.enabled`
        // UI toggle (persisted in settings.json). The `WARREN_MULTI_HOP`
        // env var is kept as a bench/power-user override so existing
        // provisioning scripts keep working. When either is on, the
        // daemon loads the signed descriptor pair from
        // `<settings_dir>/warren-multihop.json` (minted by ops via
        // `wapi admin-mint-*`). A missing file surfaces a deliberate
        // single-hop fallback; a present-but-invalid file surfaces a
        // loud warn but does not abort the boot. Subsequent toggles
        // flow through `on_set_warren_multi_hop_settings`, which reloads
        // the config and reconnects without a daemon restart.
        let warren_multi_hop_opt_in =
            settings.warren_multi_hop.enabled || warren_multi_hop_mode::is_enabled();
        let warren_multi_hop = if warren_multi_hop_opt_in {
            match warren_multi_hop::load_from_settings_dir(&config.settings_dir) {
                Ok(Some(cfg)) => {
                    log::info!(
                        "Warren multi-hop enabled ({} loaded + PKI verified)",
                        warren_multi_hop::MULTI_HOP_FILENAME
                    );
                    Some(cfg)
                }
                Ok(None) => {
                    log::warn!(
                        "Warren multi-hop is on but {} is missing in {}; falling back to single-hop",
                        warren_multi_hop::MULTI_HOP_FILENAME,
                        config.settings_dir.display()
                    );
                    None
                }
                Err(e) => {
                    log::warn!(
                        "Failed to load {}: {e}. Falling back to single-hop.",
                        warren_multi_hop::MULTI_HOP_FILENAME
                    );
                    None
                }
            }
        } else {
            None
        };

        // Live, shared Mullvad-format view of the warren-relays. Seeded
        // from the bootstrap; the background `WarrenRelayListUpdater`
        // hot-swaps it on every verified fetch. Shared via `Arc<Mutex<…>>`
        // so the pull RPC (`on_get_relay_locations`) and the broadcast
        // closure (`on_relay_list_update`) always read the *current* list
        // rather than a stale boot snapshot.
        let warren_relay_list_view: Arc<Mutex<Option<RelayList>>> =
            Arc::new(Mutex::new(warren_relay_selector.as_ref().map(|sel| {
                warren_relay_list_view::to_mullvad_relay_list(sel.list())
            })));
        if let Some(selector) = warren_relay_selector.as_ref() {
            warren_network_stats.set_relay_list(selector.list());
        }

        // Forward the resolved warren-api URL so the
        // failover path (tunnel.rs) can post a best-effort
        // exit-down report.
        let warren_api_url_for_params: Option<String> = Some(warren_api_url.clone());
        let parameters_generator = tunnel::ParametersGenerator::new_with_optional_warren(
            relay_selector.clone(),
            settings.relay_settings.clone(),
            settings.tunnel_options.clone(),
            warren_relay_selector,
            warren_signing_key,
            warren_shared_seed.clone(),
            warren_multi_hop,
            warren_status_cache.clone(),
            warren_api_url_for_params,
            warren_token_provider::RouteKemTrust {
                server_pins: warren_server_pubkey.clone().into_iter().collect(),
                remembered_at: Some(
                    config
                        .cache_dir
                        .join(warren_token_provider::REMEMBERED_ROUTE_ADMISSION),
                ),
            },
        );
        // Snapshot the persisted DAITA opt-in onto the
        // parameters generator at boot. Without this, the first
        // tunnel connect after a daemon restart would always go out
        // DAITA-off even when the user had previously toggled it on.
        // Subsequent toggles flow through `on_set_daita_*` handlers.
        {
            let initial_daita = settings.tunnel_options.wireguard.daita.enabled;
            let pg_for_daita_boot = parameters_generator.clone();
            tokio::spawn(async move {
                pg_for_daita_boot
                    .set_warren_enable_daita(initial_daita)
                    .await;
            });
        }

        // Snapshot the persisted parallel-connection preference at
        // boot, mirroring the DAITA snapshot above: without it, the
        // first connect after a daemon restart would always use the
        // compiled default even when the user persisted a custom value.
        {
            let initial_n_connections = settings.warren_n_connections;
            let pg_for_nconns_boot = parameters_generator.clone();
            tokio::spawn(async move {
                pg_for_nconns_boot
                    .set_warren_n_connections(initial_n_connections)
                    .await;
            });
        }

        // Snapshot the persisted client-side bandwidth ceiling at boot,
        // mirroring the n_connections snapshot above: without it, the
        // first connect after a daemon restart would run uncapped even
        // when the user persisted a limit.
        {
            let initial_max_rate = settings.warren_max_rate_bps;
            let pg_for_max_rate_boot = parameters_generator.clone();
            tokio::spawn(async move {
                pg_for_max_rate_boot
                    .set_warren_max_rate_bps(initial_max_rate)
                    .await;
            });
        }

        // Snapshot the persisted advanced "custom exit" override at boot,
        // mirroring the n_connections snapshot above: without it, the
        // first connect after a daemon restart would ignore a persisted
        // custom exit and fall back to roster selection.
        {
            let initial_custom_exit = settings.warren_custom_exit.clone();
            let pg_for_custom_exit_boot = parameters_generator.clone();
            tokio::spawn(async move {
                pg_for_custom_exit_boot
                    .set_warren_custom_exit(initial_custom_exit)
                    .await;
            });
        }

        // Snapshot the persisted NAT-PMP (port forwarding) setting at
        // boot, mirroring the snapshots above: without it, a daemon
        // restart with the toggle persisted ON starts the first tunnel
        // with no NatPmpConfig, so no mapping is ever requested (the UI
        // shows the toggle enabled but port forwarding stays inactive
        // until the user toggles it off and on again).
        {
            let initial_nat_pmp = nat_pmp_settings_to_runtime_cfg(&settings.warren_nat_pmp);
            let pg_for_nat_pmp_boot = parameters_generator.clone();
            tokio::spawn(async move {
                pg_for_nat_pmp_boot
                    .set_warren_nat_pmp(initial_nat_pmp)
                    .await;
            });
        }

        let param_gen = parameters_generator.clone();
        let (param_gen_tx, mut param_gen_rx) = mpsc::unbounded();
        tokio::spawn(async move {
            while let Some(tunnel_options) = param_gen_rx.next().await {
                param_gen.set_tunnel_options(&tunnel_options).await;
            }
        });
        settings.register_change_listener(move |settings| {
            let _ = param_gen_tx.unbounded_send(settings.tunnel_options.clone());
        });

        // Wire the verify-hook channel into the daemon
        // main loop. Every TOFU pin event (insert / bump / mismatch /
        // trust / reset) is forwarded as an `InternalDaemonEvent` so
        // the daemon's `handle_event` can persist it through
        // `SettingsPersister::update` and (for mismatches) push it to
        // the live `WarrenStatusCache`.
        {
            let (warren_pin_tx, mut warren_pin_rx) =
                tokio::sync::mpsc::unbounded_channel::<tunnel::WarrenPinUpdate>();
            parameters_generator
                .set_warren_pin_update_tx(Some(warren_pin_tx))
                .await;
            let internal_event_tx_pin = internal_event_tx.clone();
            tokio::spawn(async move {
                while let Some(update) = warren_pin_rx.recv().await {
                    let _ =
                        internal_event_tx_pin.send(InternalDaemonEvent::WarrenPinUpdate(update));
                }
            });
        }

        // Per-app exits: the generator resolves them and every tunnel
        // follows the plan; what the route sessions report, and each new
        // resolution, comes back here to be shown.
        let app_route_resolutions = {
            let reports_tx = internal_event_tx.clone();
            parameters_generator
                .set_app_route_observer(Arc::new(move |tunnel, reports| {
                    let _ =
                        reports_tx.send(InternalDaemonEvent::AppRouteReports { tunnel, reports });
                }))
                .await;
            // Before any tunnel can start, so the first one has a plan.
            parameters_generator
                .set_app_routing(settings.app_routing.clone())
                .await;
            let (routing_tx, mut routing_rx) =
                tokio::sync::watch::channel(settings.app_routing.clone());
            settings.register_change_listener(move |settings| {
                routing_tx.send_if_modified(|routing| {
                    let changed = *routing != settings.app_routing;
                    if changed {
                        routing.clone_from(&settings.app_routing);
                    }
                    changed
                });
            });
            // One task, so the generator sees the changes in their order.
            let routing_generator = parameters_generator.clone();
            tokio::spawn(async move {
                while routing_rx.changed().await.is_ok() {
                    let routing = routing_rx.borrow_and_update().clone();
                    routing_generator.set_app_routing(routing).await;
                }
            });
            let resolutions = parameters_generator.app_route_resolutions().await;
            let mut planned_rx = resolutions.clone();
            let planned_tx = internal_event_tx.clone();
            tokio::spawn(async move {
                while planned_rx.changed().await.is_ok() {
                    if planned_tx
                        .send(InternalDaemonEvent::AppRoutesPlanned)
                        .is_err()
                    {
                        return;
                    }
                }
            });
            resolutions
        };

        let param_gen_relay_settings = parameters_generator.clone();
        settings.register_change_listener(move |settings| {
            let settings = settings.clone();
            let param_gen = param_gen_relay_settings.clone();
            tokio::spawn(async move { param_gen.set_settings(settings).await });
        });

        // Register a listener for generic settings changes.
        // This is useful for example for updating feature indicators when the settings change.
        let settings_changed_event_sender = internal_event_tx.clone();
        settings.register_change_listener(move |_settings| {
            let _ = settings_changed_event_sender.send(InternalDaemonEvent::SettingsChanged);
        });

        // What the boot does with a secured target is settled before the
        // tunnel state machine starts, because a boot that blocks has to start
        // it in the error state. No standing has been fetched yet, so no ban
        // is known here: one learned later blocks through the standing
        // monitor, as while running.
        let secured_boot = (*target_state == TargetState::Secured).then(|| {
            warren_session_gate::SessionGate::<()>::new(boot_login).secured_boot(
                None,
                warren_env_arbitration::secured_boot_action(settings.warren_env_yield.as_ref())
                    == warren_env_arbitration::SecuredBootAction::Restore,
            )
        });

        let route_manager = RouteManagerHandle::spawn(
            #[cfg(target_os = "linux")]
            mullvad_types::TUNNEL_FWMARK,
            #[cfg(target_os = "linux")]
            mullvad_types::TUNNEL_TABLE_ID,
        )
        .await
        .map_err(Error::RouteManager)?;

        let (offline_state_tx, offline_state_rx) = mpsc::unbounded();
        #[cfg(target_os = "windows")]
        let (volume_update_tx, volume_update_rx) = mpsc::unbounded();
        let tunnel_state_machine_handle = tunnel_state_machine::spawn(
            tunnel_state_machine::InitialTunnelState {
                allow_lan: settings.allow_lan,
                lan_networks: settings.lan_networks(),
                lockdown_mode: LockdownMode::from(settings.lockdown_mode),
                dns_config: dns::addresses_from_options(&settings.tunnel_options.dns_options),
                allowed_endpoint: access_mode_handler
                    .get_current()
                    .await
                    .map_err(Error::ApiConnectionModeError)?
                    .endpoint,
                // A dial and a revocation keep the block the previous run may
                // have left; a boot that drops its secured target lets it go.
                reset_firewall: !secured_boot
                    .as_ref()
                    .is_some_and(warren_session_gate::SecuredBoot::keeps_firewall_block),
                initial_block: secured_boot
                    .as_ref()
                    .and_then(warren_session_gate::SecuredBoot::initial_block),
                split_apps: {
                    let split_apps = tunnel_split_apps(&settings.app_routing);
                    #[cfg(target_os = "linux")]
                    let split_apps = enforceable_split_apps(
                        split_apps,
                        split_tunneling_pid_manager.include_only_supported(),
                    );
                    // Whether the driver loads is only known once the tunnel
                    // state machine has started it, which checks again.
                    #[cfg(target_os = "windows")]
                    let split_apps =
                        enforceable_split_apps(split_apps, split_tunnel::INCLUDE_ONLY_READY);
                    split_apps
                },
            },
            parameters_generator.clone(),
            Arc::new(parameters_generator.clone()),
            config.log_dir,
            config.resource_dir.clone(),
            internal_event_tx.to_specialized_sender(),
            offline_state_tx,
            route_manager.clone(),
            #[cfg(target_os = "windows")]
            volume_update_rx,
            #[cfg(target_os = "linux")]
            tunnel_state_machine::LinuxNetworkingIdentifiers {
                fwmark: mullvad_types::TUNNEL_FWMARK,
                table_id: mullvad_types::TUNNEL_TABLE_ID,
                excluded_cgroup2: split_tunneling_pid_manager.excluded_cgroup(),
                net_cls: split_tunneling_pid_manager.net_cls_classid(),
                included_cgroup2: split_tunneling_pid_manager.included_cgroup(),
            },
        )
        .await
        .map_err(Error::TunnelError)?;

        // Connectivity online-edge signal: bumped on every offline→online
        // transition so the Warren multi-hop directory updater and exit-list
        // updater force an immediate refresh when connectivity returns
        // (notably on sleep/wake), instead of sitting on a stale set until
        // their next coarse periodic tick.
        let (warren_online_edge_tx, warren_online_edge_rx) = tokio::sync::watch::channel(0u64);
        warren_token_provider::wake_on_online(warren_online_edge_rx.clone());
        warren_port_entitlements::wake_on_online(warren_online_edge_rx.clone());
        api::forward_offline_state(
            api_availability.clone(),
            offline_state_rx,
            warren_online_edge_tx,
            warren_status_cache.clone(),
        );

        let relay_list_listener = management_interface.notifier().clone();
        let internal_event_tx_clone = internal_event_tx.clone();

        // In Warren tunnel mode, we replace the `RelayList` broadcast to
        // the GUI with a view built from `warren-relays.json`
        // (see `warren_relay_list_view::to_mullvad_relay_list`). The
        // upstream Mullvad `RelayListUpdater` keeps running to
        // feed the other internal consumers (API access methods,
        // bridges) that still depend on the Mullvad list, but the
        // payload exposed to the GUI comes solely from Warren.
        let warren_view_for_closure = Arc::clone(&warren_relay_list_view);
        let on_relay_list_update = move |relay_list: &RelayList| {
            // Read the *current* Warren view (hot-swapped by the updater),
            // never a boot snapshot - otherwise a late Mullvad relay-list
            // refresh would clobber the GUI with the stale (often empty)
            // boot list.
            let to_broadcast = warren_view_for_closure
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone()
                .unwrap_or_else(|| relay_list.clone());
            relay_list_listener.notify_relay_list(to_broadcast);
            let (tx, _) = oneshot::channel();
            let _ = internal_event_tx_clone.send(InternalDaemonEvent::Command(
                DaemonCommand::UpdateDefaultLocationCountry(tx),
            ));
        };

        // Immediate broadcast of the Warren view (= without waiting
        // for the first `RelayListUpdater` refresh which may arrive
        // minutes later): the GUI will display the Warren exits as
        // soon as it first connects to the management interface.
        if let Some(view) = warren_relay_list_view
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
        {
            log::info!(
                "Broadcasting Warren relay list view ({} countries) to GUI at boot",
                view.countries.len()
            );
            management_interface
                .notifier()
                .notify_relay_list(view.clone());
        }

        // `downloads_enabled = false`. The upstream Mullvad
        // relay list is never consumed (Warren is the only tunnel mode; the
        // GUI is fed the Warren view, the tunnel uses the Warren selector),
        // and warren-api does not serve the Mullvad relay endpoint, so every
        // download would 404. Keeping the updater spawned preserves the
        // override-application path and the handle wiring; only the futile
        // network fetch is disabled.
        let mut relay_list_updater = RelayListUpdater::spawn(
            relay_selector.clone(),
            api_handle.clone(),
            &config.cache_dir,
            settings.relay_overrides.clone(),
            on_relay_list_update,
            initial_relay_list,
            false,
        );

        // Notify the relay list updater when new relay IP overrides are available.
        let relay_list_updater_handle = relay_list_updater.clone();
        settings.register_change_listener(move |settings| {
            // Notify relay selector of changes to the settings/selector config
            let mut relay_list_updater = relay_list_updater_handle.clone();
            let overrides = settings.relay_overrides.clone();
            tokio::spawn(async move {
                relay_list_updater.update_overrides(overrides).await;
            });
        });

        // The version router polls the Warren update channel for signed
        // metadata. The persisted rollout seed (if any) places this client at
        // a stable position in staged rollouts; when absent the poller only
        // surfaces fully rolled-out releases.
        let version_handle = version::router::spawn_version_router(
            config.cache_dir.clone(),
            internal_event_tx.to_specialized_sender(),
            settings.show_beta_releases,
            settings.rollout_threshold_seed,
            app_upgrade_broadcast,
        );

        // No-op (Mullvad relay downloads are disabled, see
        // `RelayListUpdater::spawn(.., false)`). Kept for parity with
        // upstream boot sequencing; the Warren exit list is fetched by the
        // dedicated `WarrenRelayListUpdater` spawned just below.
        relay_list_updater.update().await;

        // Spawn the dynamic Warren exit-list updater. It
        // refreshes `GET {warren_api_url}/v2/exits` on startup +
        // periodically, verifies the signature against the pinned server
        // pubkey before caching, writes the cache atomically, and
        // hot-swaps the live selector + rebroadcasts the GUI view with no
        // daemon restart. Mirrors the upstream `RelayListUpdater` above.
        let warren_roster_updater = {
            let warren_pg = parameters_generator.clone();
            let warren_notifier = management_interface.notifier().clone();
            // Shared live view so the updater's hot-swap is visible to the
            // synchronous pull RPC (`on_get_relay_locations`) too - not just
            // the broadcast push. Without this the GUI's mount-time pull
            // keeps returning the stale (empty in dev) boot view.
            let warren_view_for_updater = Arc::clone(&warren_relay_list_view);
            let warren_stats_for_updater = warren_network_stats.clone();
            // The handle is retained to kick an immediate refresh on the
            // connectivity online edge (sleep/wake); the task also keeps
            // itself alive via its internal keepalive sender (periodic
            // refresh for the daemon's whole lifetime).
            let warren_updater = warren_relay_list_updater::WarrenRelayListUpdater::spawn(
                warren_api_url.clone(),
                &config.cache_dir,
                warren_server_pubkey.clone(),
                None,
                warren_bootstrap_generation,
                // Optional roster feature, off by default (WARREN_ROSTER_ENABLED).
                warren_roster_enabled,
                warren_roster_pin.clone(),
                // No roster bootstrap-from-disk yet: when enabled, the
                // startup refresh_roster() fetches it. Until it lands (or
                // when disabled), the live list passes through unfiltered.
                None,
                move |list| {
                    let view = warren_relay_list_view::to_mullvad_relay_list(&list);
                    warren_stats_for_updater.set_relay_list(&list);
                    let pg = warren_pg.clone();
                    let notifier = warren_notifier.clone();
                    // Hot-swap the shared view BEFORE notifying so a GUI
                    // pull that races the push observes the fresh list.
                    *warren_view_for_updater
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(view.clone());
                    tokio::spawn(async move {
                        pg.set_warren_relay_selector(
                            crate::warren_relay_selector::DaemonWarrenRelaySelector::new(list),
                        )
                        .await;
                        notifier.notify_relay_list(view);
                    });
                },
            );
            // Force an exit-list refresh on every connectivity online edge so
            // a post-wake reconnect selects from a fresh list rather than the
            // stale pre-sleep one (single-hop `no relay matches` wedge).
            let mut online_edge_updater = warren_updater.clone();
            let mut online_edge_rx = warren_online_edge_rx.clone();
            tokio::spawn(async move {
                while online_edge_rx.changed().await.is_ok() {
                    log::info!("Warren exit list: connectivity restored; refreshing now");
                    online_edge_updater.update().await;
                }
            });
            warren_updater
        };

        // Broadcast notices: a background updater fetches the
        // server-signed notice envelope, verifies it against the same
        // pinned server key as the relay list, and pushes what must be
        // displayed into the live status the GUI subscribes to. Nothing is
        // cached to disk on purpose (see the module docs): an erased
        // message must not be able to come back.
        {
            let notices_status = warren_status_cache.clone();
            warren_notices_updater::WarrenNoticesUpdater::spawn(
                warren_api_url.clone(),
                warren_server_pubkey.clone(),
                Some(mullvad_version::VERSION.to_owned()),
                move |notices| notices_status.set_notices(notices),
            );
        }

        // Forum activity badge: the same signed-artifact discipline, over
        // a document that is identical for every client. The counts are
        // pushed to the UI verbatim; the slot that turns them into a badge
        // is known only to the renderer, so nothing here can tie the
        // document to an account.
        {
            let digest_status = warren_status_cache.clone();
            warren_forum_digest_updater::WarrenForumDigestUpdater::spawn(
                warren_api_url.clone(),
                warren_server_pubkey.clone(),
                warren_status_cache.forum_digest_demand_source(),
                move |counts| digest_status.set_forum_digest(counts),
            );
        }

        // Launch announcements: the same signed-artifact discipline as the
        // notices, over its OWN envelope. It has to be its own: notice
        // verification re-serializes the canonical preimage from the
        // client's own struct, so a field a deployed client does not know
        // makes EVERY notice fail verification and that client then shows
        // nothing at all.
        //
        // The announcement can carry an offer, and a per-account code
        // cannot ride a document that is byte-identical for every caller,
        // so the campaign lookup goes through the signed SDK client the
        // daemon already holds.
        {
            let announcements_status = warren_status_cache.clone();
            let voucher_source: Option<
                std::sync::Arc<dyn warren_announcements_updater::CampaignVoucherSource>,
            > = warren_shared_seed.clone().map(|seed| {
                std::sync::Arc::new(warren_sdk_client::SharedWarrenApiClient::new(
                    warren_api_url.clone(),
                    seed,
                ))
                    as std::sync::Arc<dyn warren_announcements_updater::CampaignVoucherSource>
            });
            warren_announcements_updater::WarrenAnnouncementsUpdater::spawn(
                warren_api_url.clone(),
                warren_server_pubkey.clone(),
                Some(mullvad_version::VERSION.to_owned()),
                voucher_source,
                warren_wallet_activity.clone(),
                move |announcements| announcements_status.set_announcements(announcements),
            );
        }

        // Port-forward abuse standing: polled on the
        // token refresh cadence and fed the issuers' ban refusals, so a v7
        // client learns of a suspension without dialing an exit.
        let warren_standing_monitor = warren_shared_seed.clone().map(|seed| {
            let standing_tx = internal_event_tx.clone();
            warren_account_standing::StandingMonitor::spawn(
                std::sync::Arc::new(warren_sdk_client::SharedWarrenApiClient::new(
                    warren_api_url.clone(),
                    seed,
                )),
                Some(config.cache_dir.clone()),
                std::sync::Arc::new(move |update| {
                    let _ = standing_tx.send(InternalDaemonEvent::WarrenAccountStanding(update));
                }),
                warren_wallet_activity.clone(),
            )
        });
        parameters_generator
            .set_warren_wallet_activity(warren_wallet_activity.clone())
            .await;
        if let Some(monitor) = &warren_standing_monitor {
            let _ = warren_standing_slot.set(monitor.clone());
            parameters_generator
                .set_warren_standing_monitor(monitor.clone())
                .await;
        }

        // Dynamic multi-hop: a background updater fetches the signed
        // directory from warren-api, verifies the full trust chain
        // (server envelope -> pinned root cert -> operational descriptors),
        // selects a 2-distinct-node circuit honoring the user's country
        // hints, and pushes the assembled `MultiHopConfig` to the
        // parameters generator, requesting a reconnect when the active
        // circuit changes. The directory is signed by the same server key
        // as the relay list, so the relay-list server pin is reused; the
        // root pin comes from `warren_multi_hop_directory::resolve_root_pins`.
        //
        // `warren_multi_hop_directory_active` records whether that updater is
        // wired and owns the reconnect for circuit changes. When it is, the
        // standard "relay settings changed" restart path MUST NOT reconnect
        // on its own: it would rebuild tunnel params from the generator's
        // currently-stored (and now stale) multi-hop circuit, briefly
        // connecting to the wrong exit before the updater re-selects the
        // correct one. The directory updater is the single source of truth
        // for the active circuit and issues exactly one reconnect with the
        // freshly-selected circuit.
        let warren_multi_hop_directory_active;
        {
            let warren_mh_root_mode = warren_multi_hop_directory::root_pin_mode();
            // Fail-closed (`Unconfigured`) never produces a circuit, so the
            // updater does not own reconnects in that mode - keep the standard
            // restart path so a location change still reconnects.
            warren_multi_hop_directory_active =
                warren_mh_root_mode != warren_multi_hop_directory::RootPinMode::Unconfigured;
            // Mirror the GUI's exit-location choice (written into the
            // standard relay `location` constraint) into the multi-hop
            // exit-country hint the circuit selector reads. The GUI does
            // NOT write `warren_multi_hop.exit_country` directly, so
            // without this bridge the user's exit selection is silently
            // ignored and the selector picks a random circuit. Entry is
            // left "any": the GUI defaults the entry location to a country
            // that may not host a Warren node, and with a small fleet the
            // entry is implied by the exit (different-country rule).
            fn effective_warren_multi_hop(
                s: &mullvad_types::settings::Settings,
            ) -> mullvad_types::settings::WarrenMultiHopSettings {
                use mullvad_types::constraints::Constraint;
                use mullvad_types::relay_constraints::{
                    GeographicLocationConstraint, LocationConstraint, RelaySettings,
                };
                let mut mh = s.warren_multi_hop.clone();
                if mh.exit_country.is_empty()
                    && let RelaySettings::Normal(rc) = &s.relay_settings
                    && let Constraint::Only(LocationConstraint::Location(geo)) = &rc.location
                {
                    mh.exit_country = match geo {
                        GeographicLocationConstraint::Country(cc)
                        | GeographicLocationConstraint::City(cc, _)
                        | GeographicLocationConstraint::Hostname(cc, _, _) => cc.clone(),
                    };
                }
                mh
            }
            let (warren_mh_tx, warren_mh_rx) =
                tokio::sync::watch::channel(effective_warren_multi_hop(&settings));
            // Push the multi-hop setting to the updater on every change so
            // a toggle flip (or exit-location edit) refreshes the circuit
            // without a daemon restart. Guarded so unrelated settings
            // changes do not wake the updater.
            settings.register_change_listener(move |s| {
                let eff = effective_warren_multi_hop(s);
                warren_mh_tx.send_if_modified(|cur| {
                    if *cur != eff {
                        *cur = eff;
                        true
                    } else {
                        false
                    }
                });
            });
            let reconnect_event_tx: DaemonEventSender<DaemonCommand> =
                internal_event_tx.to_specialized_sender();
            let request_reconnect: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
                let tx = reconnect_event_tx.clone();
                let (otx, orx) = oneshot::channel();
                if tx.send(DaemonCommand::Reconnect(otx)).is_ok() {
                    // Keep the receiver alive until `on_reconnect` replies,
                    // mirroring `schedule_reconnect`. Dropping it makes the
                    // reply send fail and log a spurious "Unable to send
                    // reconnect issued to the daemon command sender".
                    tokio::spawn(async move {
                        let _ = orx.await;
                    });
                }
            });
            // ADR 36 gap-free drain path: request channel from the drain
            // reactor (through the generator) into the directory updater.
            let (warren_drain_migration_tx, warren_drain_migration_rx) =
                tokio::sync::mpsc::unbounded_channel();
            parameters_generator
                .set_warren_drain_migration_tx(Some(warren_drain_migration_tx))
                .await;
            // Publish the cold-start circuit BEFORE `run()` can dispatch the
            // boot connect. The updater's own first pass is asynchronous, so
            // it lost that race by under a millisecond on a real upgrade and
            // the tunnel started with no circuit, which fails closed on
            // `NoCircuit` and parks the host blocked with no retry (incident
            // 2026-08-08).
            let warren_boot_seed = warren_multi_hop_directory::boot_seed(
                &config.settings_dir,
                &warren_server_pubkey
                    .clone()
                    .into_iter()
                    .collect::<Vec<String>>(),
                &warren_mh_root_mode,
                &effective_warren_multi_hop(&settings),
            );
            if let Some(seed) = warren_boot_seed.as_ref() {
                parameters_generator
                    .set_warren_multi_hop(seed.circuit.clone())
                    .await;
            }
            warren_multi_hop_directory::spawn(warren_multi_hop_directory::UpdaterConfig {
                api_url: warren_api_url.clone(),
                server_pins: warren_server_pubkey.clone().into_iter().collect(),
                root_mode: warren_mh_root_mode,
                boot_seed: warren_boot_seed,
                settings_rx: warren_mh_rx,
                parameters_generator: parameters_generator.clone(),
                request_reconnect,
                on_maintenance_migration: Some({
                    let cache = warren_status_cache.clone();
                    Arc::new(move || {
                        cache.record_maintenance_migration();
                        // Time-driven flip back to inactive: re-broadcast
                        // once the display TTL elapses so the UI banner
                        // drops without polling. The extra second absorbs
                        // timer/clock skew at the boundary.
                        let cache = cache.clone();
                        tokio::spawn(async move {
                            tokio::time::sleep(
                                warren_status::MAINTENANCE_MIGRATION_TTL
                                    + std::time::Duration::from_secs(1),
                            )
                            .await;
                            cache.rebroadcast();
                        });
                    })
                }),
                settings_dir: config.settings_dir.clone(),
                online_edge_rx: Some(warren_online_edge_rx.clone()),
                drain_migration_rx: Some(warren_drain_migration_rx),
            });
        }

        let location_handler = GeoIpHandler::new(
            api_runtime.rest_handle(mullvad_api::DefaultDnsResolver),
            internal_event_tx.clone().to_specialized_sender(),
        );

        // Enforced at once, with no tunnel: a lock left by an earlier run
        // names that run's adapter, and the list may have changed since.
        #[cfg(any(windows, target_os = "linux"))]
        let app_locks = new_app_locks(&settings);

        let daemon = Daemon {
            tunnel_state: TunnelState::Disconnected {
                location: None,
                locked_down: settings.lockdown_mode,
            },
            target_state,
            #[cfg(target_os = "linux")]
            exclude_pids: split_tunneling_pid_manager,
            rx: internal_event_rx,
            tx: internal_event_tx,
            restart_lockdown_armed: false,
            reconnection_job: None,
            management_interface,
            migration_complete,
            settings,
            account_history,
            account_manager,
            access_mode_handler,
            api_runtime,
            api_handle,
            version_handle,
            relay_selector,
            relay_list_updater,
            parameters_generator,
            warren_relay_list_view,
            warren_roster_updater,
            shutdown_tasks: vec![],
            tunnel_state_machine_handle,
            #[cfg(target_os = "windows")]
            volume_update_tx,
            #[cfg(any(windows, target_os = "linux"))]
            app_locks,
            location_handler,
            nm_vpn_indicator: nm_vpn_indicator::NmVpnIndicator::new(&config.settings_dir),
            #[cfg(target_os = "macos")]
            warren_route_manager: route_manager,
            settings_dir: config.settings_dir,
            warren_status_cache,
            warren_standing_monitor,
            warren_account_standing: None,
            warren_blocked_for_ban: false,
            warren_auto_recovery_pending: false,
            warren_identity,
            warren_multi_hop_directory_active,
            warren_foreign_envs: warren_env_arbitration::seed_observations(
                warren_product_env::CURRENT,
            ),
            app_route_resolutions,
            app_route_reports: Vec::new(),
            app_route_reports_tunnel: 0,
            app_routes_published: Vec::new(),
            warren_session: warren_session_gate::SessionGate::new(boot_login),
            secured_boot,
            warren_wallet_activity,
        };

        // Cross-environment arbitration: watch every OTHER product
        // environment's daemon so this build can stand down while a
        // higher-priority one holds the machine. Prod watches the lower ones
        // too, read-only, so its GUI can say they will stand down; it never
        // issues a command to them.
        for observation in &daemon.warren_foreign_envs {
            let env = observation.env;
            let event_tx = daemon.tx.clone();
            tokio::spawn(warren_env_arbitration::watch::observe(
                env,
                move |env, state| {
                    event_tx
                        .send(InternalDaemonEvent::WarrenForeignEnvObserved { env, state })
                        .is_ok()
                },
            ));
        }

        daemon.publish_env_arbitration();

        api_availability.unsuspend();

        #[cfg(target_os = "macos")]
        {
            let account_manager = daemon.account_manager.clone();
            let commands = daemon.tx.to_specialized_sender();
            tokio::task::spawn(async {
                if let Err(error) =
                    macos::handle_app_bundle_removal(account_manager, commands).await
                {
                    log::error!("Failed to handle app removal: {error}");
                }
            });
        }

        Ok(daemon)
    }
}
