//! The split tunneling handlers: whether it is supported, its processes, apps, mode and state.

use crate::*;

impl Daemon {
    #[cfg(target_os = "windows")]
    pub(crate) fn on_split_tunnel_is_supported(&mut self, tx: oneshot::Sender<bool>) {
        Self::oneshot_send(
            tx,
            self.tunnel_state_machine_handle.split_tunnel().is_loaded(),
            "split_tunnel_is_supported response",
        );
    }

    #[cfg(target_os = "linux")]
    pub(crate) fn on_split_tunnel_is_supported(&mut self, tx: oneshot::Sender<bool>) {
        let supported = self.exclude_pids.is_supported();
        Self::oneshot_send(tx, supported, "split_tunnel_is_supported response");
    }

    /// Whether the split tunnel processes are the included ones: the PID
    /// RPCs act on the cgroup of the split mode in force, so `warren
    /// split-tunnel add` includes a process in include-only mode and
    /// excludes it otherwise.
    #[cfg(target_os = "linux")]
    pub(crate) fn split_processes_are_included(&self) -> bool {
        self.settings.app_routing.split_mode == SplitMode::IncludeOnly
    }

    #[cfg(target_os = "linux")]
    pub(crate) fn on_get_split_tunnel_processes(
        &mut self,
        tx: ResponseTx<Vec<i32>, split_tunnel::Error>,
    ) {
        let result = if self.split_processes_are_included() {
            self.exclude_pids.list_included()
        } else {
            self.exclude_pids.list()
        };
        let result = result.inspect_err(|error| {
            log::error!("{}", error.display_chain_with_msg("Unable to obtain PIDs"));
        });
        Self::oneshot_send(tx, result, "get_split_tunnel_processes response");
    }

    #[cfg(target_os = "linux")]
    pub(crate) fn on_add_split_tunnel_process(
        &mut self,
        tx: ResponseTx<(), split_tunnel::Error>,
        pid: i32,
    ) {
        let result = if self.split_processes_are_included() {
            self.exclude_pids.add_included(pid)
        } else {
            self.exclude_pids.add(pid)
        };
        let result = result.inspect_err(|error| {
            log::error!("{}", error.display_chain_with_msg("Unable to add PID"));
        });
        Self::oneshot_send(tx, result, "add_split_tunnel_process response");
    }

    #[cfg(target_os = "linux")]
    pub(crate) fn on_remove_split_tunnel_process(
        &mut self,
        tx: ResponseTx<(), split_tunnel::Error>,
        pid: i32,
    ) {
        let result = if self.split_processes_are_included() {
            self.exclude_pids.remove_included(pid)
        } else {
            self.exclude_pids.remove(pid)
        };
        let result = result.inspect_err(|error| {
            log::error!("{}", error.display_chain_with_msg("Unable to remove PID"));
        });
        Self::oneshot_send(tx, result, "remove_split_tunnel_process response");
    }

    #[cfg(target_os = "linux")]
    pub(crate) fn on_clear_split_tunnel_processes(
        &mut self,
        tx: ResponseTx<(), split_tunnel::Error>,
    ) {
        let result = if self.split_processes_are_included() {
            self.exclude_pids.clear_included()
        } else {
            self.exclude_pids.clear()
        };
        let result = result.inspect_err(|error| {
            log::error!("{}", error.display_chain_with_msg("Unable to clear PIDs"));
        });
        Self::oneshot_send(tx, result, "clear_split_tunnel_processes response");
    }

    pub(crate) async fn on_add_split_tunnel_app(&mut self, tx: ResponseTx<(), Error>, app: AppId) {
        self.update_app_routing(tx, "add_split_tunnel_app response", |routing| {
            routing.excluded_apps.insert(app);
            Ok(())
        })
        .await;
    }

    pub(crate) async fn on_remove_split_tunnel_app(
        &mut self,
        tx: ResponseTx<(), Error>,
        app: AppId,
    ) {
        self.update_app_routing(tx, "remove_split_tunnel_app response", |routing| {
            routing.excluded_apps.remove(&app);
            Ok(())
        })
        .await;
    }

    pub(crate) async fn on_clear_split_tunnel_apps(&mut self, tx: ResponseTx<(), Error>) {
        self.update_app_routing(tx, "clear_split_tunnel_apps response", |routing| {
            routing.excluded_apps.clear();
            Ok(())
        })
        .await;
    }

    /// The pre-app-routing switch: on selects exclusion, off leaves
    /// exclusion and keeps include-only, which it never knew about.
    pub(crate) async fn on_set_split_tunnel_state(
        &mut self,
        tx: ResponseTx<(), Error>,
        state: bool,
    ) {
        let current = self.settings.app_routing.split_mode;
        let mode = split_mode_for_state(current, state);
        self.update_app_routing(tx, "set_split_tunnel_state response", |routing| {
            routing.split_mode = mode;
            Ok(())
        })
        .await;
    }

    pub(crate) async fn on_set_app_split_mode(
        &mut self,
        tx: ResponseTx<(), Error>,
        mode: SplitMode,
    ) {
        self.update_app_routing(tx, "set_app_split_mode response", |routing| {
            routing.split_mode = mode;
            Ok(())
        })
        .await;
    }

    #[cfg(target_os = "windows")]
    pub(crate) fn on_get_split_tunnel_processes(
        &self,
        tx: ResponseTx<Vec<ExcludedProcess>, split_tunnel::Error>,
    ) {
        Self::oneshot_send(
            tx,
            self.tunnel_state_machine_handle
                .split_tunnel()
                .get_processes(),
            "get_split_tunnel_processes response",
        );
    }

    #[cfg(target_os = "windows")]
    pub(crate) fn on_check_volumes(&mut self, tx: ResponseTx<(), Error>) {
        if self.volume_update_tx.unbounded_send(()).is_ok() {
            let _ = tx.send(Ok(()));
        }
    }
}
