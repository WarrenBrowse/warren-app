//! The handlers that sign the community-forum requests with the wallet.

use crate::*;

impl Daemon {
    /// Signs the community-forum login approval for `sid`: the
    /// canonical request `POST /v1/forum/login` with the bound body
    /// `{"login_version":2,"sid":"<sid>"}`, using the Warren identity key held
    /// in the daemon, and returns the header values + body. Replies `None`
    /// when no signer is bootstrapped or the sid is malformed (the gRPC layer
    /// refuses one first).
    ///
    /// **No-log policy**: the `sid`, pubkey and signature are never logged.
    pub(crate) fn on_sign_forum_login(
        &self,
        tx: oneshot::Sender<Option<(mullvad_api::warren_auth::WarrenAuthHeaders, String)>>,
        sid: String,
    ) {
        let Some(body) = forum_login_body(&sid) else {
            log::debug!("sign_forum_login: not signed (malformed sid)");
            Self::oneshot_send(tx, None, "sign_forum_login");
            return;
        };
        self.sign_forum_request(tx, warren_forum::FORUM_LOGIN_PATH, body, "sign_forum_login");
    }

    /// Signs the community-forum notification read. The body is
    /// fixed and carries nothing: warren-connect derives the account from
    /// the signature, so there is no field a caller could point elsewhere.
    /// Replies `None` when no signer is bootstrapped.
    ///
    /// **No-log policy**: the pubkey and signature are never logged.
    pub(crate) fn on_sign_forum_notifications(
        &self,
        tx: oneshot::Sender<Option<(mullvad_api::warren_auth::WarrenAuthHeaders, String)>>,
    ) {
        self.sign_forum_request(
            tx,
            "/v1/forum/notifications",
            forum_notifications_body(),
            "sign_forum_notifications",
        );
    }

    /// Signs marking the caller's own forum notification list seen.
    /// Same empty body and same account derivation as the read, over its own
    /// path so the two signatures are not interchangeable.
    ///
    /// **No-log policy**: the pubkey and signature are never logged.
    pub(crate) fn on_sign_forum_notifications_seen(
        &self,
        tx: oneshot::Sender<Option<(mullvad_api::warren_auth::WarrenAuthHeaders, String)>>,
    ) {
        self.sign_forum_request(
            tx,
            "/v1/forum/notifications/seen",
            forum_notifications_body(),
            "sign_forum_notifications_seen",
        );
    }

    /// Signs the community-forum attach-logs request for `sid`.
    /// Builds the canonical body via [`forum_attach_body`] and signs
    /// `POST /v1/forum/attach-logs` with the Warren identity key held in
    /// the daemon, returning the header values + the exact signed body (the
    /// GUI POSTs it verbatim). Replies `None` when no signer is
    /// bootstrapped. The `sid` and `log_gz` are validated upstream in the
    /// gRPC layer, so they are safe to embed here.
    ///
    /// **No-log policy**: the `sid`, pubkey, signature and log content are
    /// never logged.
    pub(crate) fn on_sign_forum_attach_logs(
        &self,
        tx: oneshot::Sender<Option<(mullvad_api::warren_auth::WarrenAuthHeaders, String)>>,
        sid: String,
        topic_id: u64,
        log_gz: Vec<u8>,
    ) {
        self.sign_forum_request(
            tx,
            "/v1/forum/attach-logs",
            forum_attach_body(&sid, topic_id, &log_gz),
            "sign_forum_attach_logs",
        );
    }

    /// The one place a community-forum request is signed: reads the broker's
    /// clock, stamps the signature with it, and hands back the headers with
    /// the exact bytes the GUI POSTs. Replies `None` when no identity is
    /// bootstrapped, before anything is read or signed.
    ///
    /// The correction is what makes these flows work on a machine whose clock
    /// has never synchronised: the broker refuses a signature more than a
    /// minute from its own, and every one of these refusals is terminal for
    /// the flow that raised it. It lives here rather than in each handler so
    /// a later flow cannot be added without it.
    ///
    /// **No-log policy**: the pubkey, the signature and the body are never
    /// logged, only whether it signed.
    pub(crate) fn sign_forum_request(
        &self,
        tx: oneshot::Sender<Option<(mullvad_api::warren_auth::WarrenAuthHeaders, String)>>,
        path: &'static str,
        body: String,
        flow: &'static str,
    ) {
        if !self.warren_identity.has_user_identity() {
            log::debug!("{flow}: not signed (no identity)");
            Self::oneshot_send(tx, None, flow);
            return;
        }
        let signer = self.warren_identity.signer();
        let authorized_as = signer.pubkey_ss58();
        tokio::spawn(async move {
            let timestamp =
                warren_api_clock::forum_signing_timestamp(warren_forum::connect_host()).await;
            let signed = signed_forum_request_for(&signer, &authorized_as, path, body, timestamp);
            match signed {
                Some(_) => log::debug!("{flow}: signed"),
                None => log::debug!("{flow}: not signed (the identity moved)"),
            }
            if tx.send(signed).is_err() {
                log::warn!("Unable to send {flow} to the daemon command sender");
            }
        });
    }

    /// Signs the community-forum in-app report. Builds the
    /// canonical body via [`forum_report_body`] (the crate the mobile FFI
    /// share) and signs `POST /v1/forum/report` with the Warren identity key
    /// held in the daemon, returning the header values + the exact signed
    /// body (the GUI POSTs it verbatim).
    ///
    /// **No-log policy**: the pubkey, the signature, the report text and the
    /// log content are never logged; only whether it signed, and the class
    /// of a refusal.
    pub(crate) fn on_sign_forum_report(
        &self,
        tx: oneshot::Sender<
            Result<(mullvad_api::warren_auth::WarrenAuthHeaders, String), ForumReportSignError>,
        >,
        report_json: String,
        log_gz: Option<Vec<u8>>,
    ) {
        if !self.warren_identity.has_user_identity() {
            log::debug!("on_sign_forum_report: not signed (no identity)");
            Self::oneshot_send(
                tx,
                Err(ForumReportSignError::NoIdentity),
                "sign_forum_report",
            );
            return;
        }
        // Read the broker's clock before stamping, off the daemon loop: a
        // machine whose own clock never synchronised is otherwise refused on
        // every attempt, and a bug report is exactly what a user files when
        // nothing else in the app works. The signer stays here, so what the
        // caller can influence is nothing.
        let signer = self.warren_identity.signer();
        let authorized_as = signer.pubkey_ss58();
        tokio::spawn(async move {
            let timestamp =
                warren_api_clock::forum_signing_timestamp(warren_forum::connect_host()).await;
            let result = signed_forum_report_for(
                &signer,
                &authorized_as,
                &report_json,
                log_gz.as_deref(),
                timestamp,
            );
            match &result {
                Ok(_) => log::debug!("on_sign_forum_report: signed"),
                Err(err) => log::debug!("on_sign_forum_report: not signed ({err:?})"),
            }
            if tx.send(result).is_err() {
                log::warn!("Unable to send sign_forum_report to the daemon command sender");
            }
        });
    }
}
