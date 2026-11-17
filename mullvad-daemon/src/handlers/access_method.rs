//! The handlers of the custom lists and of the API access methods.

use crate::*;

impl Daemon {
    pub(crate) async fn on_create_custom_list(
        &mut self,
        tx: ResponseTx<mullvad_types::custom_list::Id, Error>,
        name: String,
        locations: BTreeSet<GeographicLocationConstraint>,
    ) {
        let result = self.create_custom_list(name, locations).await;
        Self::oneshot_send(tx, result, "create_custom_list response");
    }

    pub(crate) async fn on_delete_custom_list(
        &mut self,
        tx: ResponseTx<(), Error>,
        id: mullvad_types::custom_list::Id,
    ) {
        let result = self.delete_custom_list(id).await;
        Self::oneshot_send(tx, result, "delete_custom_list response");
    }

    pub(crate) async fn on_update_custom_list(
        &mut self,
        tx: ResponseTx<(), Error>,
        new_list: CustomList,
    ) {
        let result = self.update_custom_list(new_list).await;
        Self::oneshot_send(tx, result, "update_custom_list response");
    }

    pub(crate) async fn on_clear_custom_lists(&mut self, tx: ResponseTx<(), Error>) {
        let result = self.clear_custom_lists().await;
        Self::oneshot_send(tx, result, "clear_custom_lists response");
    }

    pub(crate) async fn on_add_access_method(
        &mut self,
        tx: ResponseTx<mullvad_types::access_method::Id, Error>,
        name: String,
        enabled: bool,
        access_method: AccessMethod,
    ) {
        let result = self.add_access_method(name, enabled, access_method).await;
        Self::oneshot_send(tx, result, "add_api_access_method response");
    }

    pub(crate) async fn on_remove_api_access_method(
        &mut self,
        tx: ResponseTx<(), Error>,
        api_access_method: mullvad_types::access_method::Id,
    ) {
        let result = self
            .remove_access_method(api_access_method)
            .await
            .map_err(Error::AccessMethodError);
        Self::oneshot_send(tx, result, "remove_api_access_method response");
    }

    pub(crate) async fn on_set_api_access_method(
        &mut self,
        tx: ResponseTx<(), Error>,
        access_method: mullvad_types::access_method::Id,
    ) {
        let result = self
            .use_api_access_method(access_method)
            .await
            .map_err(Error::AccessMethodError);
        Self::oneshot_send(tx, result, "set_api_access_method response");
    }

    pub(crate) async fn on_update_api_access_method(
        &mut self,
        tx: ResponseTx<(), Error>,
        method: AccessMethodSetting,
    ) {
        let result = self.update_access_method(method).await;
        Self::oneshot_send(tx, result, "update_api_access_method response");
    }

    pub(crate) async fn on_clear_custom_api_access_methods(&mut self, tx: ResponseTx<(), Error>) {
        let result = self
            .clear_custom_api_access_methods()
            .await
            .map_err(Error::AccessMethodError);
        Self::oneshot_send(tx, result, "clear_custom_api_access_methods response");
    }

    pub(crate) fn on_get_current_api_access_method(
        &mut self,
        tx: ResponseTx<AccessMethodSetting, Error>,
    ) {
        let handle = self.access_mode_handler.clone();
        tokio::spawn(async move {
            let result = handle
                .get_current()
                .await
                .map(|current| current.setting)
                .map_err(Error::ApiConnectionModeError);
            Self::oneshot_send(tx, result, "get_current_api_access_method response");
        });
    }

    pub(crate) fn on_test_proxy_as_access_method(
        &mut self,
        tx: ResponseTx<bool, Error>,
        proxy: talpid_types::net::proxy::CustomProxy,
    ) {
        use mullvad_api::proxy::{ApiConnectionMode, ProxyConfig};
        use talpid_types::net::AllowedEndpoint;

        let connection_mode = ApiConnectionMode::Proxied(ProxyConfig::from(proxy.clone()));
        let api_proxy = self.create_limited_api_proxy(connection_mode.clone());
        let proxy_endpoint = AllowedEndpoint {
            endpoint: proxy.get_remote_endpoint().endpoint,
            clients: api::allowed_clients(&connection_mode),
        };

        let daemon_event_sender = self.tx.to_specialized_sender();
        let access_method_selector = self.access_mode_handler.clone();
        tokio::spawn(async move {
            let result = Self::test_access_method(
                proxy_endpoint,
                access_method_selector,
                daemon_event_sender,
                api_proxy,
            )
            .await
            .map_err(Error::AccessMethodError);

            Self::oneshot_send(tx, result, "on_test_proxy_as_access_method response");
        });
    }

    pub(crate) async fn on_test_api_access_method(
        &mut self,
        tx: ResponseTx<bool, Error>,
        access_method: mullvad_types::access_method::Id,
    ) {
        let reply =
            |response| Self::oneshot_send(tx, response, "on_test_api_access_method response");

        let access_method = match self.get_api_access_method(access_method) {
            Ok(x) => x,
            Err(err) => {
                reply(Err(Error::AccessMethodError(err)));
                return;
            }
        };

        let test_subject = match self
            .access_mode_handler
            .resolve(access_method.clone())
            .await
        {
            Ok(Some(test_subject)) => test_subject,
            Ok(None) => {
                let error =
                    Error::ApiConnectionModeError(mullvad_api::access_mode::Error::Resolve {
                        access_method: access_method.access_method,
                    });
                reply(Err(error));
                return;
            }
            Err(err) => {
                reply(Err(Error::ApiConnectionModeError(err)));
                return;
            }
        };

        let api_proxy = self.create_limited_api_proxy(test_subject.connection_mode);
        let daemon_event_sender = self.tx.to_specialized_sender();
        let access_method_selector = self.access_mode_handler.clone();

        tokio::spawn(async move {
            let result = Self::test_access_method(
                test_subject.endpoint,
                access_method_selector,
                daemon_event_sender,
                api_proxy,
            )
            .await
            .map_err(Error::AccessMethodError);

            log::debug!(
                "API access method {method} {verdict}",
                method = test_subject.setting.name,
                verdict = match result {
                    Ok(true) => "could successfully connect to the Warren API",
                    _ => "could not connect to the Warren API",
                }
            );

            reply(result);
        });
    }
}
