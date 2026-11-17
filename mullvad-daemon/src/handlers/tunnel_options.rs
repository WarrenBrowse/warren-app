//! The handlers of the tunnel options: IPv6, DNS, DAITA, relay overrides, and the WireGuard-era settings the clients still send.

use crate::*;

impl Daemon {
    pub(crate) async fn on_set_obfuscation_settings(
        &mut self,
        tx: ResponseTx<(), settings::Error>,
        new_settings: ObfuscationSettings,
    ) {
        match self
            .settings
            .update(move |settings| settings.obfuscation_settings = new_settings)
            .await
        {
            Ok(_) => {
                Self::oneshot_send(tx, Ok(()), "set_obfuscation_settings");
            }
            Err(err) => {
                log::error!(
                    "{}",
                    err.display_chain_with_msg("Failed to set obfuscation settings")
                );
                Self::oneshot_send(tx, Err(err), "set_obfuscation_settings");
            }
        }
    }

    pub(crate) async fn on_set_enable_ipv6(
        &mut self,
        tx: ResponseTx<(), settings::Error>,
        enable_ipv6: bool,
    ) {
        match self
            .settings
            .update(|settings| settings.tunnel_options.generic.enable_ipv6 = enable_ipv6)
            .await
        {
            Ok(settings_changed) => {
                Self::oneshot_send(tx, Ok(()), "set_enable_ipv6 response");
                if settings_changed {
                    log::info!("Initiating tunnel restart because the enable IPv6 setting changed");
                    self.reconnect_tunnel();
                }
            }
            Err(e) => {
                log::error!("{}", e.display_chain_with_msg("Unable to save settings"));
                Self::oneshot_send(tx, Err(e), "set_enable_ipv6 response");
            }
        }
    }

    pub(crate) async fn on_set_userspace_wireguard(
        &mut self,
        tx: ResponseTx<(), settings::Error>,
        userspace: bool,
    ) {
        match self
            .settings
            .update(|settings| settings.tunnel_options.wireguard.userspace = userspace)
            .await
        {
            Ok(_) => {
                Self::oneshot_send(tx, Ok(()), "set_userspace_wireguard response");
            }
            Err(e) => {
                log::error!("{}", e.display_chain_with_msg("Unable to save settings"));
                Self::oneshot_send(tx, Err(e), "set_userspace_wireguard response");
            }
        }
    }

    pub(crate) async fn on_set_enable_recents(
        &mut self,
        tx: ResponseTx<(), settings::Error>,
        enable_recents: bool,
    ) {
        match self
            .settings
            .update(|settings| match settings.recents {
                None if enable_recents => {
                    settings.recents = Some(vec![]);
                    settings.update_recents();
                }
                Some(_) if !enable_recents => {
                    settings.recents = None;
                }
                _ => (),
            })
            .await
        {
            Ok(_) => {
                Self::oneshot_send(tx, Ok(()), "set_enable_recents response");
            }
            Err(e) => {
                log::error!("{}", e.display_chain_with_msg("Unable to save settings"));
                Self::oneshot_send(tx, Err(e), "set_enable_recents response");
            }
        }
    }

    pub(crate) async fn on_set_quantum_resistant_tunnel(
        &mut self,
        tx: ResponseTx<(), settings::Error>,
        quantum_resistant: QuantumResistantState,
    ) {
        match self
            .settings
            .update(|settings| {
                settings.tunnel_options.wireguard.quantum_resistant = quantum_resistant
            })
            .await
        {
            Ok(settings_changed) => {
                Self::oneshot_send(tx, Ok(()), "set_quantum_resistant_tunnel response");
                if settings_changed {
                    log::info!("Reconnecting because the PQ safety setting changed");
                    self.reconnect_tunnel();
                }
            }
            Err(e) => {
                log::error!("{}", e.display_chain_with_msg("Unable to save settings"));
                Self::oneshot_send(tx, Err(e), "set_quantum_resistant_tunnel response");
            }
        }
    }

    pub(crate) async fn on_set_daita_enabled(
        &mut self,
        tx: ResponseTx<(), settings::Error>,
        value: bool,
    ) {
        let result = self
            .settings
            .update(|settings| {
                settings.tunnel_options.wireguard.daita.enabled = value;
            })
            .await;

        match result {
            Ok(settings_changed) => {
                Self::oneshot_send(tx, Ok(()), "set_daita_enabled response");
                if let RelaySettings::CustomTunnelEndpoint(_) = &self.settings.relay_settings {
                    return; // DAITA is not supported for custom relays
                }

                // Mirror the toggle onto the parameters generator so the
                // next reconnect carries `Setup.daita_support = value` on
                // the warrenguard-wire v3 handshake.
                self.parameters_generator
                    .set_warren_enable_daita(value)
                    .await;

                if settings_changed {
                    log::info!("Reconnecting because DAITA settings changed");
                    self.reconnect_tunnel();
                }
            }
            Err(e) => {
                log::error!("{}", e.display_chain_with_msg("Unable to save settings"));
                Self::oneshot_send(tx, Err(e), "set_daita_enabled response");
            }
        }
    }

    pub(crate) async fn on_set_daita_use_multihop_if_necessary(
        &mut self,
        tx: ResponseTx<(), settings::Error>,
        value: bool,
    ) {
        match self
            .settings
            .update(|settings| {
                settings
                    .tunnel_options
                    .wireguard
                    .daita
                    .use_multihop_if_necessary = value
            })
            .await
        {
            Ok(settings_changed) => {
                Self::oneshot_send(tx, Ok(()), "set_daita_use_multihop_if_necessary response");

                if let RelaySettings::CustomTunnelEndpoint(_) = &self.settings.relay_settings {
                    return; // DAITA is not supported for custom relays
                }

                let daita_enabled = self.settings.tunnel_options.wireguard.daita.enabled;

                if settings_changed && daita_enabled {
                    log::info!("Reconnecting because DAITA settings changed");
                    self.reconnect_tunnel();
                }
            }
            Err(e) => {
                log::error!("{}", e.display_chain_with_msg("Unable to save settings"));
                Self::oneshot_send(tx, Err(e), "set_daita_use_multihop_if_necessary response");
            }
        }
    }

    pub(crate) async fn on_set_daita_settings(
        &mut self,
        tx: ResponseTx<(), settings::Error>,
        daita_settings: DaitaSettings,
    ) {
        let new_enabled = daita_settings.enabled;
        match self
            .settings
            .update(|settings| settings.tunnel_options.wireguard.daita = daita_settings)
            .await
        {
            Ok(settings_changed) => {
                Self::oneshot_send(tx, Ok(()), "set_daita_settings response");
                // Same mirror as `on_set_daita_enabled`. The
                // struct-update path can also flip `enabled`, so we
                // forward the new value here too.
                self.parameters_generator
                    .set_warren_enable_daita(new_enabled)
                    .await;
                if settings_changed {
                    log::info!("Reconnecting because DAITA settings changed");
                    self.reconnect_tunnel();
                }
            }
            Err(e) => {
                log::error!("{}", e.display_chain_with_msg("Unable to save settings"));
                Self::oneshot_send(tx, Err(e), "set_daita_settings response");
            }
        }
    }

    pub(crate) async fn on_set_dns_options(
        &mut self,
        tx: ResponseTx<(), settings::Error>,
        dns_options: DnsOptions,
    ) {
        match self
            .settings
            .update(move |settings| settings.tunnel_options.dns_options = dns_options)
            .await
        {
            Ok(settings_changed) => {
                if settings_changed {
                    let settings = self.settings.settings();
                    let resolvers =
                        dns::addresses_from_options(&settings.tunnel_options.dns_options);
                    self.send_tunnel_command(TunnelCommand::Dns(
                        resolvers,
                        oneshot_map(tx, |tx, ()| {
                            Self::oneshot_send(tx, Ok(()), "set_dns_options response");
                        }),
                    ));
                } else {
                    Self::oneshot_send(tx, Ok(()), "set_dns_options response");
                }
            }
            Err(e) => {
                log::error!("{}", e.display_chain_with_msg("Unable to save settings"));
                Self::oneshot_send(tx, Err(e), "set_dns_options response");
            }
        }
    }

    pub(crate) async fn on_set_relay_override(
        &mut self,
        tx: ResponseTx<(), settings::Error>,
        relay_override: RelayOverride,
    ) {
        match self
            .settings
            .update(move |settings| settings.set_relay_override(relay_override))
            .await
        {
            Ok(settings_changed) => {
                Self::oneshot_send(tx, Ok(()), "set_relay_override response");
                if settings_changed {
                    self.reconnect_tunnel();
                }
            }
            Err(e) => {
                log::error!("{}", e.display_chain_with_msg("Unable to save settings"));
                Self::oneshot_send(tx, Err(e), "set_relay_override response");
            }
        }
    }

    pub(crate) async fn on_clear_all_relay_overrides(
        &mut self,
        tx: ResponseTx<(), settings::Error>,
    ) {
        match self
            .settings
            .update(move |settings| settings.relay_overrides.clear())
            .await
        {
            Ok(settings_changed) => {
                Self::oneshot_send(tx, Ok(()), "clear_all_relay_overrides response");
                if settings_changed {
                    self.reconnect_tunnel();
                }
            }
            Err(e) => {
                log::error!("{}", e.display_chain_with_msg("Unable to save settings"));
                Self::oneshot_send(tx, Err(e), "clear_all_relay_overrides response");
            }
        }
    }

    pub(crate) async fn on_set_wireguard_mtu(
        &mut self,
        tx: ResponseTx<(), settings::Error>,
        mtu: Option<u16>,
    ) {
        match self
            .settings
            .update(move |settings| settings.tunnel_options.wireguard.mtu = mtu)
            .await
        {
            Ok(_) => {
                Self::oneshot_send(tx, Ok(()), "set_wireguard_mtu response");
            }
            Err(e) => {
                log::error!("{}", e.display_chain_with_msg("Unable to save settings"));
                Self::oneshot_send(tx, Err(e), "set_wireguard_mtu response");
            }
        }
    }

    pub(crate) async fn on_set_wireguard_allowed_ips(
        &mut self,
        tx: ResponseTx<(), settings::Error>,
        allowed_ips: Constraint<AllowedIps>,
    ) {
        match self
            .settings
            .update(move |settings| {
                if let RelaySettings::Normal(ref mut relay_settings) = settings.relay_settings {
                    relay_settings.wireguard_constraints.allowed_ips = allowed_ips;
                }
            })
            .await
        {
            Ok(_) => {
                Self::oneshot_send(tx, Ok(()), "set_wireguard_allowed_ips response");
            }
            Err(e) => {
                log::error!("{}", e.display_chain_with_msg("Unable to save settings"));
                Self::oneshot_send(tx, Err(e), "set_wireguard_allowed_ips response");
            }
        }
    }
}
